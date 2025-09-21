use std::{fmt, str::FromStr};

#[cfg(feature = "clap")]
use clap::ValueEnum;

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "clap", derive(Clone, ValueEnum))]
pub enum DType {
    // Float16 is not available on accelerate
    Float16,
    Float32,
}

impl fmt::Display for DType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // Float16 is not available on accelerate
            #[cfg(any(all(feature = "candle", not(feature = "accelerate"))))]
            DType::Float16 => write!(f, "float16"),
            #[cfg(any(feature = "candle", feature = "ort"))]
            DType::Float32 => write!(f, "float32"),
            #[cfg(feature = "python")]
            DType::Bfloat16 => write!(f, "bfloat16"),
            #[allow(unreachable_patterns)]
            _ => write!(f, "unknown"),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for DType {
    fn default() -> Self {
        #[cfg(any(feature = "accelerate", feature = "mkl", feature = "ort"))]
        {
            DType::Float32
        }
        #[cfg(not(any(feature = "accelerate", feature = "mkl", feature = "ort")))]
        {
            DType::Float16
        }
    }
}

impl FromStr for DType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "float16" | "f16" => {
                #[cfg(any(all(feature = "candle", not(feature = "accelerate"))))]
                {
                    Ok(DType::Float16)
                }
                #[cfg(not(any(all(feature = "candle", not(feature = "accelerate")))))]
                {
                    Err("float16 is not supported with the current backend".to_string())
                }
            }
            "float32" | "f32" => {
                #[cfg(any(feature = "candle", feature = "ort"))]
                {
                    Ok(DType::Float32)
                }
                #[cfg(not(any(feature = "candle", feature = "ort")))]
                {
                    Err("float32 is not supported with the current backend".to_string())
                }
            }
            #[allow(unreachable_patterns)]
            _ => Err(format!("Unknown dtype: {s}")),
        }
    }
}
