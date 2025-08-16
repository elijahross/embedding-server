pub mod config;
pub mod db_operations;
pub mod error;

use crate::db_operations::{process_new_files, sync_s3_files};
use crate::error::{Error, Result};
use aws_sdk_s3::Client;
use chrono::Utc;
use lib_ai::Embeddings;
use lib_core::database::ModelManager;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};
use tokio::sync::Mutex;
use tokio_cron_scheduler::{JobBuilder, JobScheduler};
use tracing::info;
use uuid::Uuid;

const JOBS_FILE: &str = "jobs.json";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JobRecord {
    pub id: String,
    pub job_type: String,
    pub cron: String,
}

#[derive(Clone)]
pub struct JobsCache {
    pub jobs: Arc<Mutex<HashMap<Uuid, (String, String)>>>,
}

impl JobsCache {
    /// Returns a serializable Vec of JobRecord for external use.
    pub async fn serializable_jobs(&self) -> Vec<JobRecord> {
        let jobs = self.jobs.lock().await;
        jobs.iter()
            .map(|(id, (job_type, cron))| JobRecord {
                id: id.to_string(),
                job_type: job_type.clone(),
                cron: cron.clone(),
            })
            .collect()
    }
}

impl JobsCache {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_job(&self, job_type: String, id: Uuid, cron: String) -> Result<()> {
        let mut jobs = self.jobs.lock().await;
        jobs.insert(id, (job_type, cron));
        save_jobs_to_file(&jobs).await?;
        Ok(())
    }

    pub async fn remove_job(&self, id: Uuid) -> Result<()> {
        let mut jobs = self.jobs.lock().await;
        jobs.remove(&id);
        save_jobs_to_file(&jobs).await?;
        Ok(())
    }

    pub async fn get_jobs(&self) -> Vec<JobRecord> {
        let jobs = self.serializable_jobs().await;
        jobs
    }

    pub async fn set_jobs(&self, map: HashMap<Uuid, (String, String)>) {
        let mut jobs = self.jobs.lock().await;
        *jobs = map;
    }
}

type BoxFutureUnit = Pin<Box<dyn Future<Output = ()> + Send>>;
type JobFn = Arc<dyn Fn() -> BoxFutureUnit + Send + Sync + 'static>;

#[derive(Clone)]
pub struct ChronJobs {
    pub scheduler: Arc<Mutex<JobScheduler>>,
    pub cache: JobsCache,
    registry: Arc<HashMap<String, JobFn>>,
}

impl ChronJobs {
    /// Build the scheduler + job registry from owned deps.
    pub async fn new(
        mm: Arc<ModelManager>,
        s3_client: Arc<Client>,
        embedder: Arc<Mutex<Embeddings>>,
    ) -> Result<Self> {
        let scheduler = Arc::new(Mutex::new(JobScheduler::new().await.map_err(|e| {
            Error::ChronFails(format!("Failed to create JobScheduler: {}", e))
        })?));
        let cache = JobsCache::new();

        // Build the registry with 'static closures that own Arcs.
        let registry = JobRegistry::build(mm, s3_client, embedder);

        Ok(Self {
            scheduler,
            cache,
            registry: Arc::new(registry),
        })
    }

    /// Start the scheduler and restore jobs from disk.
    pub async fn start(&self) -> Result<()> {
        let job_map = load_jobs_from_file().await.unwrap_or_default();
        self.cache.set_jobs(job_map.clone()).await;

        for (job_id, (job_type, cron_expr)) in job_map {
            self.add_cron_job(job_id, job_type, cron_expr).await?;
        }

        let sched = self.scheduler.lock().await;
        sched
            .start()
            .await
            .map_err(|e| Error::ChronFails(format!("Failed to start JobScheduler: {}", e)))?;
        info!("Scheduler started");
        Ok(())
    }

    /// Add & persist a new job.
    pub async fn add_job(&self, job_type: String, cron: String) -> Result<Uuid> {
        let id = Uuid::new_v4();
        self.cache
            .add_job(job_type.clone(), id, cron.clone())
            .await?;
        self.add_cron_job(id, job_type, cron).await?;
        Ok(id)
    }

    /// Remove a job by id.
    pub async fn remove_job(&self, id: Uuid) -> Result<()> {
        let sched = self.scheduler.lock().await;
        sched
            .remove(&id)
            .await
            .map_err(|e| Error::ChronFails(format!("Failed to remove job {}: {}", id, e)))?;
        self.cache.remove_job(id).await
    }

    /// Internal: create the scheduled task from the registry entry.
    pub async fn add_cron_job(&self, id: Uuid, job_type: String, cron: String) -> Result<()> {
        let job_fn = self
            .registry
            .get(&job_type)
            .cloned()
            .ok_or_else(|| Error::ChronFails(format!("No job found for type {}", job_type)))?;

        // Async run logic
        let job_id = id;
        let job_logic = Box::new(move |_jid: uuid::Uuid, mut sched: JobScheduler| {
            let job_type = job_type.clone();
            let job_fn = job_fn.clone();
            Box::pin(async move {
                info!("Job {} is running", job_type);
                // run & log errors yourself inside job_fn (we capture none here)
                (job_fn)().await;

                match sched.next_tick_for_job(job_id).await {
                    Ok(Some(ts)) => info!("Next time for job {} is {:?}", job_type, ts),
                    _ => info!("Could not get next tick for {}", job_type),
                }
            }) as BoxFutureUnit
        });

        let job = JobBuilder::new()
            .with_timezone(Utc)
            .with_job_id(id.into())
            .with_cron_job_type()
            .with_schedule(cron.clone())
            .map_err(|e| Error::ChronFails(format!("Invalid cron '{}': {}", cron, e)))?
            .with_run_async(job_logic)
            .build()
            .map_err(|e| Error::ChronFails(format!("Failed to build job: {}", e)))?;

        let sched = self.scheduler.lock().await;
        sched
            .add(job)
            .await
            .map_err(|e| Error::ChronFails(format!("Failed to add job to scheduler: {}", e)))?;
        Ok(())
    }
}

struct JobRegistry;

impl JobRegistry {
    fn build(
        mm: Arc<ModelManager>,
        client: Arc<Client>,
        embedder: Arc<Mutex<Embeddings>>,
    ) -> HashMap<String, JobFn> {
        let mut m: HashMap<String, JobFn> = HashMap::new();

        // sync_s3_files
        {
            let mm = Arc::clone(&mm);
            let client = Arc::clone(&client);
            let f: JobFn = Arc::new(move || {
                let mm = Arc::clone(&mm);
                let client = Arc::clone(&client);
                Box::pin(async move {
                    if let Err(e) = sync_s3_files(&mm, &client).await {
                        tracing::error!("sync_s3_files failed: {:?}", e);
                    }
                })
            });
            m.insert("sync_s3_files".to_string(), f);
        }

        // process_new_files
        {
            let mm = Arc::clone(&mm);
            let client = Arc::clone(&client);
            let embedder = Arc::clone(&embedder);
            let f: JobFn = Arc::new(move || {
                let mm = Arc::clone(&mm);
                let client = Arc::clone(&client);
                let embedder = Arc::clone(&embedder);
                Box::pin(async move {
                    // Lock only around the call that needs the embedder; keep critical section short
                    let emb_guard = embedder.lock().await;
                    if let Err(e) = process_new_files(&mm, &client, &*emb_guard).await {
                        tracing::error!("process_new_files failed: {:?}", e);
                    }
                })
            });
            m.insert("process_new_files".to_string(), f);
        }

        m
    }
}

async fn save_jobs_to_file(jobs: &HashMap<Uuid, (String, String)>) -> Result<()> {
    let job_list: Vec<JobRecord> = jobs
        .iter()
        .map(|(id, (job_type, cron))| JobRecord {
            id: id.to_string(),
            job_type: job_type.clone(),
            cron: cron.clone(),
        })
        .collect();

    let json = serde_json::to_string_pretty(&job_list)
        .map_err(|e| Error::Custom(format!("Failed to serialize jobs: {}", e)))?;
    tokio::fs::write(JOBS_FILE, json)
        .await
        .map_err(|e| Error::Custom(format!("Failed to write jobs file: {}", e)))?;
    Ok(())
}

async fn load_jobs_from_file() -> Result<HashMap<Uuid, (String, String)>> {
    match tokio::fs::read_to_string(JOBS_FILE).await {
        Ok(content) => {
            let job_list: Vec<JobRecord> = serde_json::from_str(&content)
                .map_err(|e| Error::Custom(format!("Failed to deserialize jobs: {}", e)))?;
            Ok(job_list
                .into_iter()
                .filter_map(|j| {
                    Uuid::parse_str(&j.id)
                        .ok()
                        .map(|id| (id, (j.job_type, j.cron)))
                })
                .collect())
        }
        Err(_) => Ok(HashMap::new()),
    }
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;
    use lib_core::_dev_utils;
    use lib_storage::create_aws_client;
    use tokio::time::Duration;
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    #[tokio::test]
    async fn test_job_record_serialization() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::TRACE)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Setting default subscriber failed");
        let db = _dev_utils::init_dev().await.unwrap();
        let mm = Arc::new(ModelManager::dev(db));
        let device = Device::Cpu;
        let embedder = Arc::new(Mutex::new(
            Embeddings::new("bert-base-uncased", device).unwrap(),
        ));
        let s3_client = Arc::new(create_aws_client().await);

        let cache_job = ChronJobs::new(mm, s3_client, embedder)
            .await
            .map_err(|_| Error::ChronFails("Failed to create ChronJobs instance".to_string()))
            .unwrap();
        cache_job
            .add_job("sync_s3_files".to_string(), "0 */10 * * * *".to_string())
            .await
            .unwrap();
        cache_job
            .add_job(
                "process_new_files".to_string(),
                "0 */15 * * * *".to_string(),
            )
            .await
            .unwrap();
        cache_job.start().await.unwrap();

        let serialized = cache_job.cache.serializable_jobs().await;
        println!("Serialized jobs: {:?}", serialized);
        tokio::time::sleep(Duration::from_secs(30)).await;
        let job_map = cache_job.cache.get_jobs().await;
        let ids: Vec<Uuid> = job_map
            .iter()
            .map(|j| Uuid::parse_str(&j.id).unwrap())
            .collect();
        for id in &ids {
            cache_job.remove_job(*id).await.unwrap();
        }
        let serialized = cache_job.cache.serializable_jobs().await;
        println!("Serialized jobs: {:?}", serialized);
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
// endregion: Unit Test
