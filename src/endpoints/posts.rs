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
    entities::post::Post,
    extractors::auth::AuthSession,
    get_conn,
    utils::{
        response::{ApiResponse, AppError, FuncError, response},
        state::ArcAppState,
        validate::ValidatedJson,
    },
};

mod get_post {
    use axum::extract::Path;

    use crate::views::for_user::{ForUserPostView, get_full_post_by_id};

    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        Path(post_id): Path<String>,
    ) -> Result<ApiResponse<ForUserPostView>, AppError> {
        let mut conn = get_conn!(state);
        let user = get_full_post_by_id(&post_id, &session.user_id, &mut conn, &state, false)
            .await
            .ok_or(FuncError::PostDoesNotExist)?;

        Ok(response(user, StatusCode::OK))
    }
}

mod create_post {

    use validator::ValidationError;

    use crate::database::posts::{create_post, get_post};

    use super::*;

    fn validate_flags_or_tags(langs: &Vec<String>) -> Result<(), ValidationError> {
        if langs.len() > 4 {
            return Err(ValidationError::new("too_many_languages"));
        }
        for lang in langs {
            if lang.len() > 22 {
                return Err(ValidationError::new("value_too_long"));
            } else if lang.len() == 0 {
                return Err(ValidationError::new("value_too_small"));
            }
        }
        Ok(())
    }

    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[validate(length(min = 0, max = 16384))]
        content: String,

        #[validate(custom(function = "validate_flags_or_tags"))]
        flags: Option<Vec<String>>,

        #[validate(custom(function = "validate_flags_or_tags"))]
        tags: Option<Vec<String>>,

        #[validate(length(min = 1, max = 32))]
        file_context_id: Option<String>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<ApiResponse<Post>, AppError> {
        let mut conn = get_conn!(state);
        let flags = payload.flags.unwrap_or_else(Vec::new);

        // Creating post
        let mut tx = create_tx!(conn);
        let post_id = create_post(
            &session.user_id,
            &payload.content,
            &flags,
            payload.tags,
            &payload.file_context_id,
            &mut tx,
        )
        .await;
        tx.commit().await.unwrap();

        // Getting post after it's created
        let post = get_post(&post_id, &mut conn, &state, false)
            .await
            .expect("Post didn't exist right after creating");

        return Ok(response(post, StatusCode::OK));
    }
}

pub fn router() -> Router<ArcAppState> {
    Router::new()
        .route("/{post_id}", get(get_post::handler))
        .route("/", post(create_post::handler))
}
