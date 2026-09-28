mod routes;
mod entities;

use axum::Router;
use dotenvy::dotenv;
use sea_orm::{Database, DatabaseConnection};

#[tokio::main]
async fn main() {
    let db = connect_database().await;

    let app = Router::new()
        .nest("/health", routes::healthcheck::health_route())
        .nest("/auth", routes::auth::auth_routes())
        .with_state(db);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

fn load_database_url() -> String {
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    database_url
}

async fn connect_database() -> DatabaseConnection {
    let database_url = load_database_url();

    // O DatabaseConnection do SeaORM guarda um pool de conexões do sqlx por dentro.
    Database::connect(&database_url)
        .await
        .expect("Failed to connect to database")
}
