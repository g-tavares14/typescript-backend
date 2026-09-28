use axum::extract::State;
use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use sea_orm::DatabaseConnection;

async fn health(State(db): State<DatabaseConnection>) -> impl IntoResponse {
    match db.ping().await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}

pub fn health_route() -> Router<DatabaseConnection> {
    Router::new().route("/", get(health))
}
