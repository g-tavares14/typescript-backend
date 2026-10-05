// POST /auth/logout (equivalente ao test/logout.test.ts).
mod common;

use axum::http::StatusCode;
use common::{TestApp, assert_unauthorized, default_user};
use serde_json::json;

const MARIA_EMAIL: &str = "maria@email.com";
const MARIA_PASSWORD: &str = "senha456";

async fn logout(app: &TestApp, token: &str) -> common::TestResponse {
    app.post("/auth/logout").bearer(token).send().await
}

async fn me(app: &TestApp, token: &str) -> common::TestResponse {
    app.get("/users/me").bearer(token).send().await
}

#[tokio::test]
async fn responde_204_sem_corpo() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = logout(&app, &token).await;

    assert_eq!(response.status, StatusCode::NO_CONTENT);
    assert_eq!(response.body, "");
}

#[tokio::test]
async fn invalida_o_token_users_me_e_um_segundo_logout_respondem_401() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    logout(&app, &token).await;

    assert_unauthorized(&me(&app, &token).await);
    assert_unauthorized(&logout(&app, &token).await);
}

#[tokio::test]
async fn um_novo_login_depois_do_logout_gera_um_token_que_funciona() {
    let app = TestApp::new().await;
    let old_token = app.register_and_login().await;
    logout(&app, &old_token).await;

    let new_token = app
        .login(common::DEFAULT_EMAIL, common::DEFAULT_PASSWORD)
        .await;

    assert_eq!(me(&app, &new_token).await.status, StatusCode::OK);
}

#[tokio::test]
async fn o_logout_de_um_usuario_nao_afeta_o_token_de_outro() {
    let app = TestApp::new().await;
    let token_a = app.register_and_login().await;
    app.register(json!({ "username": "maria", "email": MARIA_EMAIL, "password": MARIA_PASSWORD }))
        .await;
    let token_b = app.login(MARIA_EMAIL, MARIA_PASSWORD).await;

    logout(&app, &token_a).await;

    assert_eq!(me(&app, &token_a).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(me(&app, &token_b).await.status, StatusCode::OK);
}

#[tokio::test]
async fn sai_de_todos_os_dispositivos_dois_tokens_do_mesmo_usuario_morrem_com_um_logout() {
    // Arrange: dois logins = dois "dispositivos". Ambos valem antes do logout.
    let app = TestApp::new().await;
    app.register(default_user()).await;
    let phone = app
        .login(common::DEFAULT_EMAIL, common::DEFAULT_PASSWORD)
        .await;
    let laptop = app
        .login(common::DEFAULT_EMAIL, common::DEFAULT_PASSWORD)
        .await;
    assert_eq!(me(&app, &phone).await.status, StatusCode::OK);
    assert_eq!(me(&app, &laptop).await.status, StatusCode::OK);

    logout(&app, &phone).await;

    assert_unauthorized(&me(&app, &phone).await);
    assert_unauthorized(&me(&app, &laptop).await);
}

#[tokio::test]
async fn responde_401_sem_o_header_authorization() {
    let app = TestApp::new().await;

    assert_unauthorized(&app.post("/auth/logout").send().await);
}

#[tokio::test]
async fn responde_401_com_authorization_invalido() {
    let app = TestApp::new().await;
    let cases = [
        ("esquema Basic", "Basic am9hbzpzZW5oYTEyMw=="),
        ("Bearer sem token", "Bearer "),
        ("token que não é JWT", "Bearer nao-e-um-jwt"),
    ];

    for (case, authorization) in cases {
        let response = app
            .post("/auth/logout")
            .header("authorization", authorization)
            .send()
            .await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED, "{case}");
        assert_unauthorized(&response);
    }
}
