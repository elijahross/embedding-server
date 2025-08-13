pub mod db_operations;
pub mod error;
pub mod config;

use crate::error::{Error, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
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

#[derive(Clone)]
pub struct ChronJobs {
    pub scheduler: Arc<Mutex<JobScheduler>>,
    pub cache: JobsCache,
}

impl ChronJobs {
    pub async fn new() -> Result<Self> {
        let scheduler = Arc::new(Mutex::new(JobScheduler::new().await?));
        let cache = JobsCache::new();

        Ok(Self { scheduler, cache })
    }

    pub async fn start(&self) -> Result<()> {
        // Load and restore jobs from file
        let job_map = load_jobs_from_file().await.unwrap_or_default();
        self.cache.set_jobs(job_map.clone()).await;

        for (job_id, (job_type, cron_expr)) in job_map {
            self.add_cron_job(job_id, job_type, cron_expr).await?;
        }

        let scheduler = self.scheduler.clone();
        let sched = scheduler.lock().await;

        sched.start().await?;
        info!("Scheduler started");

        Ok(())
    }

    pub async fn add_job(&self, job_type: String, cron: String) -> Result<Uuid> {
        let id = Uuid::new_v4();
        self.cache
            .add_job(job_type.to_string(), id, cron.clone())
            .await?;
        self.add_cron_job(id, job_type, cron).await?;
        Ok(id)
    }

    pub async fn remove_job(&self, id: Uuid) -> Result<()> {
        let sched = self.scheduler.lock().await;
        sched.remove(&id).await?;
        self.cache.remove_job(id).await?;
        Ok(())
    }

    pub async fn add_cron_job(&self, id: Uuid, job_type: String, cron: String) -> Result<()> {
        let scheduler = self.scheduler.clone();
        let job_id = id;
        let registry = job_registry();

        let job_fn = registry.get(&job_type).cloned()
            .ok_or_else(|| Error::ChronFails(format!("No job found for type {}", job_type)))?;


        // Create the async job logic
        let job_logic = Box::new(
            move |_: uuid::Uuid, mut l: tokio_cron_scheduler::JobScheduler| {
                let job_type = job_type.clone();
                let job_fn = job_fn.clone();
                Box::pin(async move {
                    info!("Job {} is running", job_type);
                    job_fn().await;
                    match l.next_tick_for_job(job_id).await {
                        Ok(Some(ts)) => info!("Next time for job is {:?}", ts),
                        _ => info!("Could not get next tick"),
                    }
                })
                    as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
            },
        );

        // Build the job using JobBuilder
        let job = JobBuilder::new()
            .with_timezone(Utc)
            .with_job_id(job_id.into())
            .with_cron_job_type()
            .with_schedule(cron)?
            .with_run_async(job_logic)
            .build()?; // <–– Will return a `JobLocked`

        let sched = scheduler.lock().await;
        sched.add(job).await?;
        Ok(())
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

    let json = serde_json::to_string_pretty(&job_list)?;
    tokio::fs::write(JOBS_FILE, json)
        .await
        .map_err(|_| Error::ChronFails("Failed to write jobs file".to_string()))?;
    Ok(())
}
async fn load_jobs_from_file() -> Result<HashMap<Uuid, (String, String)>> {
    match tokio::fs::read_to_string(JOBS_FILE).await {
        Ok(content) => {
            let job_list: Vec<JobRecord> = serde_json::from_str(&content)?;
            Ok(job_list
                .into_iter()
                .map(|j| (Uuid::parse_str(&j.id).unwrap(), (j.job_type, j.cron)))
                .collect())
        }
        Err(_) => Ok(HashMap::new()),
    }
}

type JobFn = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync
>;

fn job_registry() -> HashMap<String, JobFn> {
    let mut m: HashMap<String, JobFn> = HashMap::new();

    m.insert(
        "scrape_agents".to_string(),
        Arc::new(|| Box::pin(async {
            scraping::scrape_agents().await;
        })),
    );

    m.insert(
        "send_email".to_string(),
        Arc::new(|| Box::pin(async {
            db_operations::send_email_batch().await;
        })),
    );

    m
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_job_record_serialization() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::TRACE)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Setting default subscriber failed");

        let cache_job = ChronJobs::new()
            .await
            .map_err(|_| Error::ChronFails("Failed to create ChronJobs instance".to_string()))
            .unwrap();
        cache_job
            .add_job("update_db".to_string(), "*/10  * * * * *".to_string())
            .await
            .unwrap();
        cache_job
            .add_job("clean_old_values".to_string(), "*/15  * * * * *".to_string())
            .await
            .unwrap();
        cache_job.start().await.unwrap();

        let serialized = cache_job.cache.serializable_jobs().await;
        println!("Serialized jobs: {:?}", serialized);
        tokio::time::sleep(Duration::from_secs(20)).await;
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
        tokio::time::sleep(Duration::from_secs(15)).await;
    }
}
// endregion: Unit Test