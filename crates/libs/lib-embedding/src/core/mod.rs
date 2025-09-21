use crate::error::Result;
#[cfg(feature = "clap")]
use clap::ValueEnum;
use nohash_hasher::IntMap;
use serde::Deserialize;
use std::{fmt, str::FromStr};

#[derive(Debug)]
pub struct Batch {
    pub input_ids: Vec<u32>,
    pub token_type_ids: Vec<u32>,
    pub position_ids: Vec<u32>,
    pub cumulative_seq_lengths: Vec<u32>,
    pub max_length: u32,
    pub pooled_indices: Vec<u32>,
    pub raw_indices: Vec<u32>,
}

impl Batch {
    pub fn len(&self) -> usize {
        self.cumulative_seq_lengths.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub enum Embedding {
    Pooled(Vec<f32>),
    All(Vec<Vec<f32>>),
}

pub type Embeddings = IntMap<usize, Embedding>;
pub type Predictions = IntMap<usize, Vec<f32>>;

pub trait InferenceBackend {
    fn health(&self) -> Result<()>;
    fn max_batch_size(&self) -> Option<usize> {
        None
    }

    fn is_padded(&self) -> bool;

    fn embed(&self, batch: Batch) -> Result<Embeddings>;

    fn predict(&self, batch: Batch) -> Result<Predictions>;
}

#[derive(Debug, PartialEq, Clone)]
pub enum ModelType {
    Classifier,
    Embedding(Pool),
}

#[derive(Debug, PartialEq, Clone, Deserialize)]
#[cfg_attr(feature = "clap", derive(ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum Pool {
    /// Some Transformer models (like BERT) use a special [CLS] token at the start of the sequence.
    Cls,
    /// Takes the embeddings of all tokens in the sequence and computes their average (mean pooling)
    Mean,
    /// SPLADE stands for Sparse Lexical and Expansion Model for Information Retrieval.
    /// This only works if your backend loaded a Masked Language Model (ForMaskedLM), since SPLADE uses vocabulary-level logits to build sparse vectors.
    /// Typically used for search / retrieval tasks
    Splade,
    /// Select the last token as embedding. This is often used in autoregressive models (like GPT-style models), where the last token embedding is meaningful for prediction.
    LastToken,
}

impl fmt::Display for Pool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Pool::Cls => write!(f, "cls"),
            Pool::Mean => write!(f, "mean"),
            Pool::Splade => write!(f, "splade"),
            Pool::LastToken => write!(f, "last_token"),
        }
    }
}

impl FromStr for Pool {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cls" => Ok(Pool::Cls),
            "mean" => Ok(Pool::Mean),
            "splade" => Ok(Pool::Splade),
            "last_token" => Ok(Pool::LastToken),
            _ => Err(format!("Invalid pooling method: {}", s)),
        }
    }
}
