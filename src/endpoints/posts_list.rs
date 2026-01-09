use axum::{Router, extract::State, http::StatusCode, routing::get};
use serde::Deserialize;
use validator::Validate;

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

mod popular {
    use crate::database::posts_list::{PostsList, get_popular_posts};

    use super::*;

    #[derive(Debug, Validate, Deserialize)]
    pub struct Params {
        hide_viewed: Option<bool>,

        #[validate(length(min = 1, max = 128))]
        cursor: Option<String>,

        #[validate(range(min = 1, max = 1000))]
        limit: Option<i64>,
    }

    pub async fn handler(
        session: AuthSession,
        State(state): State<ArcAppState>,
        ValidatedQuery(params): ValidatedQuery<Params>,
    ) -> Result<ApiResponse<PostsList>, AppError> {
        let mut conn = get_conn!(state);
        let posts = get_popular_posts(
            &session.user_id,
            &mut conn,
            params.limit.unwrap_or(50),
            params.cursor,
            params.hide_viewed.unwrap_or(false),
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
    Router::new().route("/popular", get(popular::handler))
}
