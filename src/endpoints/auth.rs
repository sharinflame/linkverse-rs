use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    create_tx,
    database::conn::LazyConn,
    get_conn,
    utils::{
        response::{ApiResponse, AppError, FuncError, response},
        security::check_password_async,
        state::ArcAppState,
        validate::{ValidatedJson, validate_username},
    },
};

/// Login endpoint
mod login {
    use crate::database::auth::{Tokens, create_tokens, get_auth_user_by_email};

    use super::*;

    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[validate(length(min = 8))]
        password: String,

        #[validate(email)]
        email: String,
    }

    pub async fn handler(
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<ApiResponse<Tokens>, AppError> {
        let mut conn = get_conn!(state);

        // Getting user
        let user = get_auth_user_by_email(&payload.email, &mut conn)
            .await
            .ok_or(FuncError::UserNotFound)?;

        // Checking password
        let correct = check_password_async(
            user.password_hash.unwrap_or("".to_string()),
            payload.password,
        )
        .await;
        if !correct {
            return Err(FuncError::IncorrectPassword.into());
        }

        // Generating tokens
        let mut tx = create_tx!(conn);
        let tokens = create_tokens(user.user_id, &mut tx).await;
        tx.commit().await.unwrap();

        return Ok(response(tokens, StatusCode::OK));
    }
}

/// Register endpoint
mod register {
    use crate::database::auth::{
        Tokens, create_tokens, create_user, email_exists, username_exists,
    };

    use super::*;

    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[validate(length(min = 8))]
        password: String,

        #[validate(email)]
        email: String,

        #[validate(custom(function = "validate_username"))]
        username: String,
    }

    pub async fn handler(
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<ApiResponse<Tokens>, AppError> {
        let mut conn = get_conn!(state);

        // Check existence of email
        if email_exists(&payload.email, &mut conn).await {
            return Err(FuncError::UserAlreadyExists.into());
        }

        // Check existence of username
        if username_exists(&payload.username, &mut conn).await {
            return Err(FuncError::UsernameExists.into());
        }

        // Creating new user and tokens
        let mut tx = create_tx!(conn);
        let user_id =
            create_user(&payload.username, &payload.email, payload.password, &mut tx).await;
        let tokens = create_tokens(user_id, &mut tx).await;
        tx.commit().await.unwrap();

        Ok(response(tokens, StatusCode::OK))
    }
}

mod check {
    use axum::extract::Query;

    use super::*;
    use crate::database::auth::{email_exists, username_exists};

    #[derive(Debug, Deserialize)]
    pub struct Params {
        r#type: String,
        value: String,
    }

    pub async fn handler(
        State(state): State<ArcAppState>,
        Query(params): Query<Params>,
    ) -> Result<StatusCode, AppError> {
        let mut conn = get_conn!(state);

        // Check existence of email
        if params.r#type == "email" && email_exists(&params.value, &mut conn).await {
            return Err(FuncError::UserAlreadyExists.into());
        }

        // Check existence of username
        if params.r#type == "username" {
            if !validate_username(&params.value).is_ok() {
                return Err(FuncError::IncorrectData.into());
            }
            if username_exists(&params.value, &mut conn).await {
                return Err(FuncError::UsernameExists.into());
            }
        }

        Ok(StatusCode::NO_CONTENT)
    }
}

mod me {
    use crate::{
        database::auth::get_auth_user, entities::user::AuthUser, extractors::auth::AuthSession,
    };

    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
    ) -> Result<ApiResponse<AuthUser>, AppError> {
        let mut conn = get_conn!(state);
        let user = get_auth_user(&session.user_id, &mut conn)
            .await
            .ok_or(FuncError::UserNotFound)?;

        Ok(response(user, StatusCode::OK))
    }
}

mod refresh {
    use crate::{
        database::auth::{Tokens, check_session_secret, update_tokens},
        utils::security::decode_token,
    };

    use super::*;
    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        refresh_token: String,
    }

    pub async fn handler(
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<ApiResponse<Tokens>, AppError> {
        // Decode token
        let decoded = decode_token(&payload.refresh_token, Some("refresh"))
            .map_err(|e| AppError::Unauthorized(e))?;

        // Check if it's expired
        if decoded.is_expired {
            return Err(FuncError::ExpiredToken.into());
        }

        // Check existence of session and check secret key
        let mut conn = get_conn!(state);
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

        // Create new tokens
        let mut tx = create_tx!(conn);
        let tokens = update_tokens(decoded.user_id, decoded.session_id, &mut tx).await;
        tx.commit().await.unwrap();

        Ok(response(tokens, StatusCode::OK))
    }
}

mod logout {
    use crate::{database::auth::remove_session, extractors::auth::AuthSession};

    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
    ) -> Result<StatusCode, AppError> {
        let mut conn = get_conn!(state);

        // TODO (future): Send logout requests to WS

        let mut tx = create_tx!(conn);
        remove_session(&session.session_id, &session.user_id, &mut tx).await;
        tx.commit().await.unwrap();
        Ok(StatusCode::NO_CONTENT)
    }
}

// TODO: Add email and password endpoints

pub fn router() -> Router<ArcAppState> {
    Router::new()
        .route("/login", post(login::handler))
        .route("/register", post(register::handler))
        .route("/check", get(check::handler))
        .route("/me", get(me::handler))
        .route("/refresh", post(refresh::handler))
        .route("/logout", post(logout::handler))
}
