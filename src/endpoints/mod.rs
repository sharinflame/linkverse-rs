use axum::Router;

use crate::utils::state::ArcAppState;

mod auth;
mod posts;
mod posts_list;
mod users;

pub fn create_router() -> Router<ArcAppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/users", users::router())
        .nest("/posts", posts::router())
        .nest("/posts", posts_list::router())
}
