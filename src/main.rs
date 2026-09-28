mod routes;

use axum::Router;
use dotenvy::dotenv;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    let pool = create_pool().await;

    let app = Router::new()
        .nest("/health", routes::healthcheck::health_route())
        .nest("/auth", routes::auth::auth_routes())
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

fn load_database_url() -> String {
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    database_url
}

async fn create_pool() -> PgPool {
    let database_url = load_database_url();
    let pool = PgPoolOptions::new()
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    pool
}
