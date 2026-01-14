use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use serde_with::{DisplayFromStr, serde_as};
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
        Path(post_id): Path<i64>,
    ) -> Result<ApiResponse<ForUserPostView>, AppError> {
        let mut conn = get_conn!(state);
        let post = get_full_post_by_id(&post_id, &session.user_id, &mut conn, false)
            .await
            .ok_or(FuncError::PostDoesNotExist)?;

        Ok(response(post, StatusCode::OK))
    }
}

mod batch_get_post {
    use serde::Serialize;
    use serde_with::skip_serializing_none;

    use crate::{
        utils::{format::parse_numbers, validate::ValidatedQuery},
        views::for_user::{ForUserPostView, get_full_post_by_id},
    };

    use super::*;

    #[derive(Debug, Deserialize, Validate)]
    pub struct Params {
        #[serde(deserialize_with = "parse_numbers")]
        posts: Vec<i64>,
    }

    #[serde_as]
    #[derive(Debug, Serialize)]
    pub struct BatchError {
        #[serde_as(as = "DisplayFromStr")]
        pub post: i64,
        pub error: &'static str,
    }

    #[derive(Debug, Serialize)]
    #[skip_serializing_none]
    pub struct Returns {
        pub posts: Vec<ForUserPostView>,
        pub errors: Option<Vec<BatchError>>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedQuery(params): ValidatedQuery<Params>,
    ) -> Result<ApiResponse<Returns>, AppError> {
        let mut conn = get_conn!(state);
        let mut posts: Vec<ForUserPostView> = Vec::with_capacity(params.posts.len());
        let mut errors: Option<Vec<BatchError>> = None;

        for post_id in params.posts {
            let post = get_full_post_by_id(&post_id, &session.user_id, &mut conn, false).await;
            if let Some(post) = post {
                posts.push(post);
            } else {
                errors.get_or_insert_with(Vec::new).push(BatchError {
                    post: post_id,
                    error: "POST_DOES_NOT_EXIST",
                });
            }
        }

        if posts.is_empty() {
            return Ok(ApiResponse::err(
                Some(Returns { posts, errors }),
                "BATCH_FAILED",
                StatusCode::BAD_REQUEST,
            ));
        }

        Ok(response(Returns { posts, errors }, StatusCode::OK))
    }
}

mod create_post {
    use validator::ValidationError;

    use crate::database::posts::{create_post, get_post};

    use super::*;

    fn validate_flags_or_tags(langs: &Vec<String>) -> Result<(), ValidationError> {
        if langs.len() > 4 {
            return Err(ValidationError::new("too_many_values"));
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

    #[serde_as]
    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[validate(length(min = 0, max = 16384))]
        content: String,

        #[validate(custom(function = "validate_flags_or_tags"))]
        flags: Option<Vec<String>>,

        #[validate(custom(function = "validate_flags_or_tags"))]
        tags: Option<Vec<String>>,

        #[serde_as(as = "Option<DisplayFromStr>")]
        #[validate(range(min = 1))]
        file_context_id: Option<i64>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<ApiResponse<Post>, AppError> {
        let mut conn = get_conn!(state);
        let flags = payload.flags.unwrap_or_else(Vec::new);

        // TODO: Check if context exists

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
        let post = get_post(&post_id, &mut conn, false)
            .await
            .expect("Post didn't exist right after creating");

        return Ok(response(post, StatusCode::CREATED));
    }
}

mod view_posts {
    use crate::database::posts::mark_posts_as_viewed;

    use super::*;

    #[serde_as]
    #[derive(Debug, Deserialize, Validate)]
    pub struct Payload {
        #[serde_as(as = "Vec<DisplayFromStr>")]
        #[validate(length(min = 1, max = 50))]
        posts: Vec<i64>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedJson(payload): ValidatedJson<Payload>,
    ) -> Result<StatusCode, AppError> {
        let mut conn = get_conn!(state);

        let mut tx = create_tx!(conn);
        mark_posts_as_viewed(&session.user_id, &payload.posts, &mut tx).await;
        tx.commit().await.unwrap();

        return Ok(StatusCode::NO_CONTENT);
    }
}

mod delete_post {
    use axum::extract::Path;

    use crate::{
        database::{
            posts::{get_post, mark_post_as_deleted},
            users::get_permissions,
        },
        utils::perms::Permission,
    };

    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        Path(post_id): Path<i64>,
    ) -> Result<StatusCode, AppError> {
        let mut conn = get_conn!(state);
        let post = get_post(&post_id, &mut conn, false)
            .await
            .ok_or(FuncError::PostDoesNotExist)?;

        if post.user_id != session.user_id {
            let perms = get_permissions(&session.user_id, &mut conn).await;
            if perms.contains(Permission::MODERATE_POSTS) {
                // TODO: Finish moderation deletion
                return Err(FuncError::NotImplemented.into());
            }
            return Err(FuncError::Forbidden.into());
        }

        let mut tx = create_tx!(conn);
        mark_post_as_deleted(&post_id, &mut tx).await;
        tx.commit().await.unwrap();

        Ok(StatusCode::NO_CONTENT)
    }
}

pub fn router() -> Router<ArcAppState> {
    Router::new()
        .route(
            "/{post_id}",
            get(get_post::handler).delete(delete_post::handler),
        )
        .route("/batch", get(batch_get_post::handler))
        .route("/", post(create_post::handler))
        .route("/view", post(view_posts::handler))
}
