use axum::{Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;

use crate::{
    create_tx,
    database::conn::LazyConn,
    entities::user::User,
    extractors::auth::AuthSession,
    get_conn,
    utils::{
        response::{ApiResponse, AppError, FuncError, response},
        state::ArcAppState,
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

pub fn router() -> Router<ArcAppState> {
    Router::new().route("/{post_id}", get(get_post::handler))
}
