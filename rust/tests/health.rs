// Testes de integração: cada arquivo em tests/ é compilado como um crate separado, que usa o nosso crate
// (`meu_backend`) como uma biblioteca, sem acesso ao que não for `pub`.
mod common;

use std::time::Duration;

use axum::{body::Body, http::Request, http::StatusCode};
use common::TestApp;
use meu_backend::{app::build_app, state::AppState};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt; // traz o método `.oneshot()` para o Router

#[tokio::test]
async fn health_responde_200_com_o_banco_de_pe() {
    let app = TestApp::new().await;

    let response = app.get("/health").send().await;

    assert_eq!(response.status, StatusCode::OK);
}

#[tokio::test]
async fn health_responde_503_sem_banco() {
    // Porta 1: nada escuta ali. `connect_lazy` não conecta agora, só na primeira consulta (que vai falhar).
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(1))
        .connect_lazy("postgres://app:app@127.0.0.1:1/nada")
        .unwrap();
    let app = build_app(AppState::new(
        pool,
        "segredo-de-teste-com-pelo-menos-32-caracteres",
    ));

    // `oneshot` manda uma requisição direto para o Router, sem abrir porta (como o app.inject() do Fastify).
    let request = Request::get("/health").body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
