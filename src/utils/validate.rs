use crate::utils::response::{AppError, FuncError};

use axum::{
    Json,
    extract::{FromRequest, Query, Request},
};
use regex::Regex;
use serde::de::DeserializeOwned;
use validator::{Validate, ValidationError};

pub struct ValidatedJson<T>(pub T);

impl<B, T> FromRequest<B> for ValidatedJson<T>
where
    B: Send + Sync + 'static,
    T: DeserializeOwned + Validate,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &B) -> Result<Self, Self::Rejection> {
        let Json(payload) = Json::<T>::from_request(req, &state)
            .await
            .map_err(|_| FuncError::IncorrectData)?;

        payload.validate().map_err(|_| FuncError::IncorrectData)?;

        Ok(ValidatedJson(payload))
    }
}

pub struct ValidatedQuery<T>(pub T);

impl<B, T> FromRequest<B> for ValidatedQuery<T>
where
    B: Send + Sync + 'static,
    T: DeserializeOwned + Validate,
{
    type Rejection = AppError;

    async fn from_request(req: Request, _state: &B) -> Result<Self, Self::Rejection> {
        let Query(payload) = Query::<T>::from_request(req, _state)
            .await
            .map_err(|_| FuncError::IncorrectParams)?;

        payload.validate().map_err(|_| FuncError::IncorrectParams)?;

        Ok(ValidatedQuery(payload))
    }
}

pub fn validate_username(nickname: &str) -> Result<(), ValidationError> {
    let re = Regex::new(r"^[A-Za-z0-9._]+$").unwrap();
    if !re.is_match(nickname) {
        return Err(ValidationError::new("invalid_username"));
    }

    if nickname.contains("..") || nickname.contains("__") {
        return Err(ValidationError::new("invalid_username"));
    }

    Ok(())
}
