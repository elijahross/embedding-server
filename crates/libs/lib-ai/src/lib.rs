pub mod error;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use error::Result;
use hf_hub::{Repo, RepoType, api::sync::Api};
use std::sync::Arc;
use tokenizers::Tokenizer;

pub struct Embeddings {
    pub device: Device,
    pub tokenizer: Arc<Tokenizer>,
    pub model: BertModel,
}

impl Embeddings {
    pub fn new(model_id: &str, device: Device) -> Result<Self> {
        let api = Api::new()?;
        let repo = api.repo(Repo::with_revision(
            model_id.to_string(),
            RepoType::Model,
            "main".to_string(),
        ));

        let tokenizer_path = repo.get("tokenizer.json")?;
        let config_path = repo.get("config.json")?;
        let model_path = repo.get("model.safetensors")?;

        let mut tokenizer = Tokenizer::from_file(tokenizer_path)?;
        tokenizer.with_padding(None).with_truncation(None)?;
        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = serde_json::from_str(&config_str)?;

        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[model_path], DType::F32, &device)? };
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            device,
            tokenizer: Arc::new(tokenizer),
            model,
        })
    }

    pub fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let tokenizer = Arc::clone(&self.tokenizer);
        let encodings = tokenizer.encode_batch(texts.to_vec(), true)?;

        let batch_size = encodings.len();
        let max_len = encodings.iter().map(|e| e.len()).max().unwrap_or(0);

        let input_ids_vec: Vec<i64> = encodings
            .iter()
            .flat_map(|e| {
                let mut ids = e.get_ids().iter().map(|&id| id as i64).collect::<Vec<_>>();
                ids.resize(max_len, 0);
                ids.into_iter()
            })
            .collect();

        let attention_mask_vec: Vec<i64> = encodings
            .iter()
            .flat_map(|e| {
                let mut mask = e.get_attention_mask().to_vec();
                mask.resize(max_len, 0);
                mask.into_iter().map(|v| v as i64)
            })
            .collect();

        let input_ids = Tensor::from_vec(input_ids_vec, (batch_size, max_len), &self.device)?;
        let attention_mask =
            Tensor::from_vec(attention_mask_vec, (batch_size, max_len), &self.device)?;
        let token_type_ids = input_ids.zeros_like()?;

        let last_hidden_state =
            self.model
                .forward(&input_ids, &token_type_ids, Some(&attention_mask))?;

        let mask_f32 = attention_mask.to_dtype(DType::F32)?;
        let mask_expanded = mask_f32
            .unsqueeze(2)?
            .broadcast_as(last_hidden_state.shape())?;
        let masked = last_hidden_state * mask_expanded;
        let summed = masked?.sum(1)?;
        let counts = mask_f32.sum(1)?.unsqueeze(1)?;

        let pooled = summed.broadcast_div(&(counts + 1e-10)?)?; // avoid div by zero
        let norms = pooled.sqr()?.sum(1)?.sqrt()?.unsqueeze(1)?;
        let norms_eps = (norms + 1e-10)?;
        let normalized = pooled.broadcast_div(&norms_eps)?;

        Ok(normalized.to_vec2::<f32>()?)
    }

    pub fn similarity(
        &self,
        embeddings1: &[Vec<f32>],
        embeddings2: &[Vec<f32>],
    ) -> Result<Vec<Vec<f32>>> {
        // embeddings1: (n, dim), embeddings2: (m, dim)
        let _n = embeddings1.len();
        let _m = embeddings2.len();
        let _dim = embeddings1[0].len();

        let emb1_tensor = Tensor::from_vec(
            embeddings1.concat(),
            (embeddings1.len(), embeddings1[0].len()), // 2D
            &self.device,
        )?;

        let emb2_tensor = Tensor::from_vec(
            embeddings2.concat(),
            (embeddings2.len(), embeddings2[0].len()), // 2D
            &self.device,
        )?;

        // Similarity = emb1 @ emb2.T
        let sims = emb1_tensor.matmul(&emb2_tensor.transpose(0, 1)?)?;
        let sims_vec: Vec<Vec<f32>> = sims.to_vec2::<f32>()?;
        Ok(sims_vec)
    }
}

// region: Unit Test
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_embeddings_model() -> Result<()> {
        let device = Device::Cpu;
        let model = Embeddings::new("intfloat/multilingual-e5-base", device)?;
        let texts = vec!["Hello world", "This is a test"];
        let embeddings = model.embed(&texts)?;
        assert_eq!(embeddings.len(), texts.len());
        println!("Embeddings shape: {:?}", embeddings[0].len());
        Ok(())
    }
}

// endregion: Unit Test
