use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};

async fn health() -> impl IntoResponse {
    StatusCode::OK
}
pub fn health_route() -> Router {
    Router::new().route("/", get(health))
}
