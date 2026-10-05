// Testes de integração: cada arquivo em tests/ é compilado como um crate separado, que usa o nosso crate
// (`meu_backend`) como uma biblioteca, sem acesso ao que não for `pub`.
use axum::{body::Body, http::Request, http::StatusCode};
use meu_backend::app::build_app;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tower::ServiceExt; // traz o método `.oneshot()` para o Router

// Mesmo banco de testes do vitest (`.env.test`), lido da raiz do repositório.
fn test_database_url() -> String {
    dotenvy::from_filename_override("../.env.test").expect(".env.test não encontrado");
    std::env::var("DATABASE_URL").expect("DATABASE_URL não definida no .env.test")
}

async fn get_health(pool: sqlx::PgPool) -> StatusCode {
    let app = build_app(pool);
    // `oneshot` manda uma requisição direto para o Router, sem abrir porta (como o app.inject() do Fastify).
    let request = Request::get("/health").body(Body::empty()).unwrap();
    app.oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn health_responde_200_com_o_banco_de_pe() {
    let pool = PgPoolOptions::new()
        .connect(&test_database_url())
        .await
        .unwrap();

    assert_eq!(get_health(pool).await, StatusCode::OK);
}

#[tokio::test]
async fn health_responde_503_sem_banco() {
    // Porta 1: nada escuta ali. `connect_lazy` não conecta agora, só na primeira consulta (que vai falhar).
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(1))
        .connect_lazy("postgres://app:app@127.0.0.1:1/nada")
        .unwrap();

    assert_eq!(get_health(pool).await, StatusCode::SERVICE_UNAVAILABLE);
}
