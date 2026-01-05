use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::{
    database::{auth::check_session_secret, conn::LazyConn},
    get_conn,
    utils::{
        response::{AppError, FuncError},
        security::decode_token,
        state::ArcAppState,
    },
};

#[derive(Debug)]
pub struct AuthSession {
    pub user_id: i64,
    pub session_id: i64,
}

impl FromRequestParts<ArcAppState> for AuthSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ArcAppState,
    ) -> Result<Self, Self::Rejection> {
        let app = state.clone();

        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(FuncError::Unauthorized)?;

        let decoded = decode_token(token, Some("access")).map_err(|e| AppError::Unauthorized(e))?;
        if decoded.is_expired {
            return Err(FuncError::ExpiredToken.into());
        }

        let mut conn = get_conn!(app);
        let is_valid = check_session_secret(
            &decoded.user_id,
            &decoded.session_id,
            &decoded.secret,
            &mut conn,
        )
        .await;
        if !is_valid {
            return Err(FuncError::InvalidToken.into());
        }

        Ok(AuthSession {
            user_id: decoded.user_id,
            session_id: decoded.session_id,
        })
    }
}
