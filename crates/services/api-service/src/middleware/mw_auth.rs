//! This module provides middleware functions and utility functions for
//! authentication and authorization in an Axum application.

use crate::config::auth_config;
use crate::error::{Error, Result};
use axum::extract::{FromRequestParts, State};
use axum::http::{Request, request::Parts};
use axum::{body::Body, middleware::Next, response::Response};
use lib_auth::bearer::{ContentToHash, hash_key};
use lib_core::model::user::Role;
use lib_core::{ctx::Ctx, database::ModelManager};
use serde::{Deserialize, Serialize};
use tower_governor::{errors::GovernorError, key_extractor::KeyExtractor};

// Governor Key Extractor
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct UserToken;
impl KeyExtractor for UserToken {
    type Key = String;

    fn extract<B>(&self, req: &Request<B>) -> std::result::Result<Self::Key, GovernorError> {
        req.headers()
            .get("Authorization")
            .and_then(|token| token.to_str().ok())
            .and_then(|token| token.strip_prefix("Bearer "))
            .map(|token| token.trim().to_owned())
            .ok_or(GovernorError::UnableToExtractKey)
    }
    fn key_name(&self, key: &Self::Key) -> Option<String> {
        Some(key.to_string())
    }
    fn name(&self) -> &'static str {
        "API_Key"
    }
}

/// This structure is used to extract path parameters from the request.
#[derive(Deserialize)]
pub struct Params {
    //put searchParams here
}

/// This custom type wraps a context with database pool from the core library.
#[derive(Debug, Clone)]
pub struct Ctm(pub Ctx);

impl<S: Send + Sync> FromRequestParts<S> for Ctm {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {
        parts
            .extensions
            .get::<Result<Ctm>>()
            .ok_or(Error::InvalidTokenFromCtx)?
            .clone()
    }
}

/// This middleware function checks the validity of the provided `Ctm` context.
/// It logs an info message and calls the next middleware in the chain if the
/// context is valid. Otherwise, it returns an error.
pub async fn request_auth(ctx: Result<Ctm>, req: Request<Body>, next: Next) -> Result<Response> {
    ctx?;
    Ok(next.run(req).await)
}

pub async fn ctx_resolver(
    State(api_key): State<Option<String>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response> {
    if let Some(stored_key) = api_key {
        // Extract API Key from Header
        let provided_key = UserToken
            .extract(&req)
            .map_err(|_| Error::UnableToExtractKey)?;

        /* // Use userId, if additional permission levels are required
        let user_id = user_extractor(&req).map_err(|_| Error::UnableToExtractKey);
        if let Ok(user_id) = user_id {
            let salt = auth_config().hash_salt;
            let content = ContentToHash {
                content: key.to_string(),
                salt,
            };
            let hashed_key = hash_key(content)?;
            let stored_key = auth_config().api_key.to_string();
            if hashed_key != stored_key {
                //return Err(Error::AuthenticationFails("Invalid API Key".to_string()));
            }

            // Store signature in context
            req.extensions_mut().insert(Ok::<Ctm, Error>(Ctm(Ctx::new(
                user_id.to_string(),
                Some(Role::Admin),
            )?)));
        } else {
            //return Err(Error::AuthenticationFails("Invalid API Key".to_string()));
        }
        */
        if provided_key != stored_key {
            return Err(Error::AuthenticationFails("Invalid API Key".to_string()));
        }
        req.extensions_mut().insert(Ok::<Ctm, Error>(Ctm(Ctx::new(
            "root".to_string(),
            Some(Role::Admin),
        )?)));
    } else {
        req.extensions_mut().insert(Ok::<Ctm, Error>(Ctm(Ctx::new(
            "root".to_string(),
            Some(Role::Admin),
        )?)));
    }
    Ok(next.run(req).await)
}

/// Extracts the API key from the request headers.
/// Returns an error if the key cannot be extracted.
pub fn user_extractor<B>(req: &Request<B>) -> Result<String> {
    req.headers()
        .get("User-X-Token")
        .and_then(|token| token.to_str().ok())
        .and_then(|token| token.strip_prefix("UserId "))
        .map(|token| token.trim().to_owned())
        .ok_or(Error::InvalidTokenFromCtx)
}
