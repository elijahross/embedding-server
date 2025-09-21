#[cfg(feature = "accelerate")]
extern crate accelerate_src;

#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

use crate::core::Batch;
use candle_core::{Result, Tensor};

mod bert;
mod dense;
mod distilbert;
mod gemma3;
mod gte;
mod mistral;
mod nomic;
mod qwen3;

#[cfg(feature = "cuda")]
mod flash_bert;

#[cfg(feature = "cuda")]
mod flash_distilbert;

#[cfg(feature = "cuda")]
mod flash_gte;

#[cfg(feature = "cuda")]
mod flash_mistral;

#[cfg(feature = "cuda")]
mod flash_nomic;

#[cfg(feature = "cuda")]
mod flash_qwen3;

pub use bert::{BertConfig, BertModel, PositionEmbeddingType};
pub use dense::{Dense, DenseConfig, DenseLayer};
pub use distilbert::{DistilBertConfig, DistilBertModel};
pub use gemma3::{Gemma3Config, Gemma3Model};
pub use gte::{GTEConfig, GTEModel};
pub use mistral::MistralConfig;
pub use nomic::{NomicBertModel, NomicConfig};
pub use qwen3::{Qwen3Config, Qwen3Model};

#[cfg(feature = "cuda")]
pub use flash_bert::FlashBertModel;

#[cfg(feature = "cuda")]
pub use flash_distilbert::FlashDistilBertModel;

#[cfg(feature = "cuda")]
pub use flash_gte::FlashGTEModel;

#[cfg(feature = "cuda")]
pub use flash_mistral::FlashMistralModel;

#[cfg(feature = "cuda")]
pub use flash_nomic::FlashNomicBertModel;

#[cfg(feature = "cuda")]
pub use flash_qwen3::FlashQwen3Model;

pub(crate) trait Model {
    fn is_padded(&self) -> bool;

    fn embed(&self, _batch: Batch) -> Result<(Option<Tensor>, Option<Tensor>)> {
        candle_core::bail!("`embed` is not implemented for this model");
    }

    fn predict(&self, _batch: Batch) -> Result<Tensor> {
        candle_core::bail!("`predict` is not implemented for this model");
    }
}
