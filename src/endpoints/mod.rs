use axum::Router;

use crate::utils::state::ArcAppState;

mod auth;
mod comments;
mod posts;
mod posts_list;
mod users;

pub fn create_router() -> Router<ArcAppState> {
    Router::new()
        .merge(auth::router())
        .merge(users::router())
        .merge(posts::router())
        .merge(posts_list::router())
        .merge(comments::router())
}
