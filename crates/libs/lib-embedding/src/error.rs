pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub enum Error {
    MissingEnv(&'static str),
    WrongFormat(&'static str),
    HuggingFaceApiError(String),
    TokenizerError(String),
    CandleError(String),
    Custom(String),
    NoBackend,
    Start(String),
    Inference(String),
    Unhealthy,
    WeightsNotFound(String),
    HuggingFaceApiErrorSync(String),
    HuggingFaceApiErrorTokio(String),
}

// region:    --- Error Boilerplate
impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}

impl From<hf_hub::api::sync::ApiError> for Error {
    fn from(err: hf_hub::api::sync::ApiError) -> Self {
        Error::HuggingFaceApiErrorSync(format!("HF Hub API Error: {err}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Custom(format!("JSON parsing error: {err}"))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Custom(format!("IO error: {err}"))
    }
}

impl From<tokenizers::Error> for Error {
    fn from(err: tokenizers::Error) -> Self {
        Error::TokenizerError(format!("Tokenizer error: {err}"))
    }
}

impl From<candle_core::Error> for Error {
    fn from(err: candle_core::Error) -> Self {
        Error::CandleError(format!("Candle core error: {err}"))
    }
}

impl From<lib_core::error::Error> for Error {
    fn from(err: lib_core::error::Error) -> Self {
        Error::Custom(err.to_string())
    }
}
impl From<hf_hub::api::tokio::ApiError> for Error {
    fn from(err: hf_hub::api::tokio::ApiError) -> Self {
        Error::HuggingFaceApiErrorTokio(format!("HF Hub API Error: {err}"))
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for Error {
    fn from(err: tokio::sync::oneshot::error::RecvError) -> Self {
        Error::Custom(format!("Channel receive error: {err}"))
    }
}

impl From<ndarray::ShapeError> for Error {
    fn from(err: ndarray::ShapeError) -> Self {
        Error::Custom(format!("NDArray shape error: {err}"))
    }
}

impl From<ort::Error> for Error {
    fn from(err: ort::Error) -> Self {
        Error::Custom(format!("ONNX Runtime error: {err}"))
    }
}

impl From<ort::ErrorCode> for Error {
    fn from(err: ort::ErrorCode) -> Self {
        Error::Custom(format!("ONNX Runtime error code: {err:?}"))
    }
}
