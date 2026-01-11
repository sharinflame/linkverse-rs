use std::{any, sync::Arc, time::Duration};

use crate::utils::{
    response::{AppError, FuncError},
    state::{AppState, CONFIG},
};
use axum::{
    Router,
    body::Body,
    http::{HeaderName, StatusCode},
    response::IntoResponse,
    routing::get,
};
use dotenvy::dotenv;
use tower_http::{
    catch_panic::CatchPanicLayer,
    cors::{Any, CorsLayer},
};
use tracing::{error, info};
use tracing_subscriber;

use axum::http::Response;

mod database;
mod endpoints;
mod entities;
mod extractors;
mod services;
mod utils;
mod views;

const ALLOWED_HEADERS: [HeaderName; 8] = [
    HeaderName::from_static("upgrade"),
    HeaderName::from_static("connection"),
    HeaderName::from_static("sec-websocket-key"),
    HeaderName::from_static("sec-websocket-version"),
    HeaderName::from_static("origin"),
    HeaderName::from_static("sec-websocket-protocol"),
    HeaderName::from_static("content-type"),
    HeaderName::from_static("authorization"),
];

fn panic_handler(err: Box<dyn any::Any + Send + 'static>) -> Response<Body> {
    let msg = if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    };
    error!("PANIC: {}", msg);

    AppError::from(FuncError::InternalServerError).into_response()
}

async fn ping() -> StatusCode {
    StatusCode::NO_CONTENT
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    dotenv().ok();

    tracing_subscriber::fmt::fmt()
        .with_writer(std::io::stderr)
        .init();

    let state = match AppState::create_from_env().await {
        Ok(state) => state,
        Err(err) => {
            error!("Failed to create AppState: {:?}", err);
            return;
        }
    };
    let shared_state = Arc::new(state);

    let v1_router: Router<()> = endpoints::create_router()
        .route("/ping", get(ping).post(ping))
        .with_state(shared_state.clone());
    let router = Router::new().nest("/v1", v1_router).layer(
        tower::ServiceBuilder::new()
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(ALLOWED_HEADERS)
                    .max_age(Duration::from_secs(86400)),
            )
            .layer(CatchPanicLayer::custom(panic_handler)),
    );

    let listener = tokio::net::TcpListener::bind(CONFIG.url.clone())
        .await
        .unwrap();
    info!("Listening on {:?}", CONFIG.url);
    axum::serve(listener, router).await.unwrap();
}
