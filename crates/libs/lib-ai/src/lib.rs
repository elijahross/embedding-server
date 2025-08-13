mod config;
pub mod error;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertConfig, BertModel};
use error::Result;
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

pub struct Embeddings {
    device: Device,
    tokenizer: Tokenizer,
    model: BertModel,
}

impl Embeddings {
    /// Initialize the embedding model and tokenizer
    pub fn new(model_id: &str) -> Result<Self> {
        let device = Device::cuda_if_available();
        let api = Api::new()?;
        let repo = api.repo(Repo::with_revision(
            model_id.to_string(),
            RepoType::Model,
            "main".to_string(),
        ));

        let tokenizer_path = repo.get("tokenizer.json")?;
        let config_path = repo.get("config.json")?;
        let model_path = repo.get("model.safetensors")?;

        let tokenizer = Tokenizer::from_file(tokenizer_path).unwrap();
        let config_str = std::fs::read_to_string(config_path)?;
        let config: BertConfig = serde_json::from_str(&config_str)?;

        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[model_path], DType::F32, &device)? };
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            device,
            tokenizer,
            model,
        })
    }

    /// Generate embeddings for a batch of texts
    pub fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let encoding = self.tokenizer.encode_batch(texts.to_vec(), true).unwrap();
        let max_len = encoding.iter().map(|e| e.len()).max().unwrap();

        let input_ids: Vec<_> = encoding
            .iter()
            .map(|e| {
                let mut ids = e.get_ids().to_vec();
                ids.resize(max_len, 0);
                ids
            })
            .collect();
        let attention_mask: Vec<_> = encoding
            .iter()
            .map(|e| {
                let mut mask = e.get_attention_mask().to_vec();
                mask.resize(max_len, 0);
                mask
            })
            .collect();

        let input_ids = Tensor::new(&input_ids, &self.device)?.to_dtype(DType::I64)?;
        let attention_mask = Tensor::new(&attention_mask, &self.device)?.to_dtype(DType::I64)?;

        let output = self
            .model
            .forward(&input_ids, Some(&attention_mask), None)?;
        let last_hidden_state = output.hidden_states.unwrap();

        let pooled = average_pool(&last_hidden_state, &attention_mask)?;
        let norms = pooled.sqr()?.sum(1)?.sqrt()?.unsqueeze(1)?;
        let normalized = pooled.broadcast_div(&norms)?;
        Ok(normalized.to_vec2::<f32>()?)
    }

    /// Compute cosine similarity matrix between two embedding sets
    pub fn similarity(&self, a: &[Vec<f32>], b: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let a_mat = nalgebra::DMatrix::from_row_slice(a.len(), a[0].len(), &a.concat());
        let b_mat = nalgebra::DMatrix::from_row_slice(b.len(), b[0].len(), &b.concat());

        let sim = &a_mat * b_mat.transpose();
        sim.row_iter()
            .map(|row| row.iter().map(|v| *v).collect())
            .collect()
    }
}

fn average_pool(last_hidden_state: &Tensor, attention_mask: &Tensor) -> Result<Tensor> {
    let mask_expanded = attention_mask.unsqueeze(2)?.to_dtype(DType::F32)?;
    let masked_hidden = last_hidden_state * &mask_expanded;
    let sum_hidden = masked_hidden.sum(1)?;
    let counts = attention_mask.sum(1)?.unsqueeze(1)?;
    sum_hidden.broadcast_div(&counts)
}
