mod routes;

use axum::Router;
use dotenvy::dotenv;

#[tokio::main]
async fn main() {
    let _database_url = load_database_url();

    let app = Router::new().nest("/health", routes::healthcheck::health_route());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

fn load_database_url() -> String {
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    database_url
}
