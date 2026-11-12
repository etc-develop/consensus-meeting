use crate::routers::{admin::admin_router, api::api_router};
use axum::{Router, extract::DefaultBodyLimit};
use tower_http::cors::CorsLayer;

pub mod state;
pub use state::{AppState, create_app_state};

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            state
                .config
                .cors_allowed_origins
                .iter()
                .map(|origin| origin.parse().expect("valid origin"))
                .collect::<Vec<_>>(),
        )
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PATCH,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ])
        .allow_credentials(true);
    Router::new()
        .merge(api_router())
        .merge(admin_router())
        .layer(DefaultBodyLimit::max(1024 * 1024 * 32))
        .layer(cors)
        .with_state(state)
}
