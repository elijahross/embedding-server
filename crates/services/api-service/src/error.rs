pub type Result<T> = core::result::Result<T, Error>;
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Clone, utoipa::ToSchema, Serialize)]
pub enum Error {
    FailToB64uDecode,
    InvalidTokenFromCtx,
    UnableToExtractKey,
    AuthenticationFails(String),
    MissingEnv(&'static str),
    WrongFormat(&'static str),
    FailToDateParse(String),
    Custom(String),
    SerdeFail(String),
}

// region:    --- Error Boilerplate
impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = StatusCode::INTERNAL_SERVER_ERROR;

        Response::builder()
            .status(status)
            .body(Body::from(format!("{self:?}")))
            .unwrap()
    }
}

impl std::error::Error for Error {}

impl From<lib_utils::error::Error> for Error {
    fn from(err: lib_utils::error::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<lib_embedding::error::Error> for Error {
    fn from(err: lib_embedding::error::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<lib_core::error::Error> for Error {
    fn from(err: lib_core::error::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<lib_cron::error::Error> for Error {
    fn from(err: lib_cron::error::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<lib_auth::error::Error> for Error {
    fn from(err: lib_auth::error::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::SerdeFail(err.to_string())
    }
}

impl From<axum::Error> for Error {
    fn from(err: axum::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<candle_core::Error> for Error {
    fn from(err: candle_core::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<tokenizers::Error> for Error {
    fn from(err: tokenizers::Error) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        Error::Custom(err.to_string())
    }
}

impl From<hf_hub::api::tokio::ApiError> for Error {
    fn from(err: hf_hub::api::tokio::ApiError) -> Self {
        Error::Custom(err.to_string())
    }
}
