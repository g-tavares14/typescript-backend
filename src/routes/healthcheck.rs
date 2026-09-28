use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use axum::extract::State;
use sqlx::PgPool;

async fn health(State(pool): State<PgPool>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
pub fn health_route() -> Router<PgPool> {
    Router::new().route("/", get(health))
}
