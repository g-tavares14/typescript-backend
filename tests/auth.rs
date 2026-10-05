// Autenticação antes do corpo e com o banco fora do ar (equivalente ao test/require-auth.test.ts).
mod common;

use std::time::Duration;

use axum::{body::Body, http::Request, http::StatusCode, http::header};
use http_body_util::BodyExt;
use meu_backend::{app::build_app, state::AppState};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

const SECRET: &str = "segredo-de-teste-com-pelo-menos-32-caracteres";

#[tokio::test]
async fn falha_do_banco_na_autenticacao_da_500_generico_sem_vazar_detalhes() {
    // Arrange: token bem assinado (passa na verificação), mas o banco não existe: a falha é na consulta da
    // token_version, dentro do extractor. Credenciais fictícias, para conferir que não vazam na resposta.
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(1))
        .connect_lazy("postgres://usuario:senha-secreta@127.0.0.1:1/inexistente")
        .unwrap();
    let state = AppState::new(pool, SECRET);
    let token = state.tokens.create_access_token(Uuid::new_v4(), 0).unwrap();
    let request = Request::get("/users/me")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .unwrap();

    // Act
    let response = build_app(state).oneshot(request).await.unwrap();

    // Assert
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(text, r#"{"error":"Erro interno do servidor"}"#);
    assert!(!text.contains("senha-secreta") && !text.contains("127.0.0.1"));
}

// O CurrentUser é o primeiro extractor das rotas protegidas: quem não está autenticado recebe 401 sem que o corpo
// seja lido, mesmo quando ele é inválido.
#[tokio::test]
async fn sem_token_e_com_json_malformado_da_401_e_nao_400() {
    let app = common::TestApp::new().await;

    for url in ["/transactions", "/auth/logout"] {
        let response = app
            .post(url)
            .header("content-type", "application/json")
            .raw_body("{ruim")
            .send()
            .await;
        common::assert_unauthorized(&response);
    }
}

#[tokio::test]
async fn com_token_valido_json_malformado_continua_sendo_400() {
    let app = common::TestApp::new().await;
    let token = app.register_and_login().await;

    let response = app
        .post("/transactions")
        .header("content-type", "application/json")
        .bearer(&token)
        .raw_body("{ruim")
        .send()
        .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
}
