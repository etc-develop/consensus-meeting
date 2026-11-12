use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use crate::models::ErrorResponse;

pub mod admin;
pub mod auth;
pub mod public;

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

impl AppError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse { error: self.message });
        (self.status, body).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, value.to_string())
    }
}

impl From<axum_extra::extract::multipart::MultipartError> for AppError {
    fn from(value: axum_extra::extract::multipart::MultipartError) -> Self {
        Self::new(StatusCode::BAD_REQUEST, value.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, value.to_string())
    }
}

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true, "service": "consensus-meeting" }))
}
