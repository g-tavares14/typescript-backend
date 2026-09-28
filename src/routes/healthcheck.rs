use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};

pub fn register() -> Router {
    Router::new().route("/", get(health_check))
}

async fn health_check() -> impl IntoResponse {
    todo!("Implement the health check endpoint")
}