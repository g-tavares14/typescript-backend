use axum::{Json, Router, routing::post};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
struct RegisterRequest {
    email: String,
    username: String,
    password: String,
}

async fn register(Json(payload): Json<RegisterRequest>) -> String {
    format!("Welcome {}!", payload.username)
}

pub fn auth_routes() -> Router<PgPool> {
    Router::new().route("/register", post(register))
}
