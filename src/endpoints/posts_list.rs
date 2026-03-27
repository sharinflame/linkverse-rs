use axum::{Router, extract::State, http::StatusCode, routing::get};
use serde::Deserialize;
use validator::Validate;

use crate::database::posts_list::{PostsList, PostsMode, get_posts};
use crate::{
    database::conn::LazyConn,
    extractors::auth::AuthSession,
    get_conn,
    utils::{
        response::{ApiResponse, AppError, FuncError, response},
        state::ArcAppState,
        validate::ValidatedQuery,
    },
};

static DEFAULT_LIMIT: i64 = 50;
static DEFAULT_HIDE_VIEWED: bool = false;

#[derive(Debug, Validate, Deserialize)]
pub struct DefaultParams {
    hide_viewed: Option<bool>,

    #[validate(length(min = 1, max = 128))]
    cursor: Option<String>,

    #[validate(range(min = 1, max = 1000))]
    limit: Option<i64>,
}

mod popular {
    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedQuery(params): ValidatedQuery<DefaultParams>,
    ) -> Result<ApiResponse<PostsList>, AppError> {
        let mut conn = get_conn!(state);
        let posts = get_posts(
            PostsMode::Popular,
            &session.user_id,
            &mut conn,
            params.limit.unwrap_or(DEFAULT_LIMIT),
            params.cursor,
            params.hide_viewed.unwrap_or(DEFAULT_HIDE_VIEWED),
        )
        .await
        .map_err(AppError::BadRequest)?;

        if posts.posts.is_empty() {
            return Err(FuncError::NoMorePosts.into());
        }

        Ok(response(posts, StatusCode::OK))
    }
}

mod following {
    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedQuery(params): ValidatedQuery<DefaultParams>,
    ) -> Result<ApiResponse<PostsList>, AppError> {
        let mut conn = get_conn!(state);
        let posts = get_posts(
            PostsMode::ByFollowing,
            &session.user_id,
            &mut conn,
            params.limit.unwrap_or(DEFAULT_LIMIT),
            params.cursor,
            params.hide_viewed.unwrap_or(DEFAULT_HIDE_VIEWED),
        )
        .await
        .map_err(AppError::BadRequest)?;

        if posts.posts.is_empty() {
            return Err(FuncError::NoMorePosts.into());
        }

        Ok(response(posts, StatusCode::OK))
    }
}

mod new {
    use super::*;

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedQuery(params): ValidatedQuery<DefaultParams>,
    ) -> Result<ApiResponse<PostsList>, AppError> {
        let mut conn = get_conn!(state);
        let posts = get_posts(
            PostsMode::New,
            &session.user_id,
            &mut conn,
            params.limit.unwrap_or(DEFAULT_LIMIT),
            params.cursor,
            params.hide_viewed.unwrap_or(DEFAULT_HIDE_VIEWED),
        )
        .await
        .map_err(AppError::BadRequest)?;

        if posts.posts.is_empty() {
            return Err(FuncError::NoMorePosts.into());
        }

        Ok(response(posts, StatusCode::OK))
    }
}

pub fn router() -> Router<ArcAppState> {
    Router::new()
        .route("/v1/posts/popular", get(popular::handler))
        .route("/v1/posts/new", get(new::handler))
        .route("/v1/posts/following", get(following::handler))
}
