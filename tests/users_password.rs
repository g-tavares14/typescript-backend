// PUT /users/me/password (equivalente ao test/users-password.test.ts). A corrida (logout no meio da troca) fica em
// tests/users.rs.
mod common;

use axum::http::StatusCode;
use common::{
    DEFAULT_EMAIL, DEFAULT_PASSWORD, TestApp, TestResponse, assert_bad_request, assert_unauthorized,
};
use serde_json::{Value, json};

const NEW_PASSWORD: &str = "outraSenha456";
const REQUIRED: &str = "Campo obrigatório ausente ou inválido";
const SHORT: &str = "A senha deve ter no mínimo 8 caracteres";
const INVALID_BODY: &str = "Corpo da requisição inválido: envie um objeto JSON";

async fn put_password(app: &TestApp, token: &str, body: Value) -> TestResponse {
    app.put("/users/me/password")
        .bearer(token)
        .json(&body)
        .send()
        .await
}

fn change(current: &str, new: &str) -> Value {
    json!({ "currentPassword": current, "newPassword": new })
}

async fn me_status(app: &TestApp, token: &str) -> StatusCode {
    app.get("/users/me").bearer(token).send().await.status
}

async fn login(app: &TestApp, password: &str) -> TestResponse {
    app.post("/auth/login")
        .json(&json!({ "email": DEFAULT_EMAIL, "password": password }))
        .send()
        .await
}

// ---- Caminho feliz ----

#[tokio::test]
async fn senha_atual_certa_da_200_com_token_novo_no_formato_do_login() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = put_password(&app, &token, change(DEFAULT_PASSWORD, NEW_PASSWORD)).await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    let new_token = body["token"].as_str().unwrap();
    assert_eq!(
        body,
        json!({ "token": new_token, "tokenType": "Bearer", "expiresIn": 3600 })
    );
    assert_ne!(new_token, token);
    assert_eq!(me_status(&app, new_token).await, StatusCode::OK);
}

#[tokio::test]
async fn depois_da_troca_os_tokens_antigos_de_todos_os_dispositivos_dao_401() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let other_device = app.login(DEFAULT_EMAIL, DEFAULT_PASSWORD).await;

    put_password(&app, &token, change(DEFAULT_PASSWORD, NEW_PASSWORD)).await;

    assert_unauthorized(&app.get("/users/me").bearer(&token).send().await);
    assert_unauthorized(&app.get("/users/me").bearer(&other_device).send().await);
}

#[tokio::test]
async fn depois_da_troca_login_com_a_senha_nova_entra_e_com_a_antiga_nao() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    put_password(&app, &token, change(DEFAULT_PASSWORD, NEW_PASSWORD)).await;

    assert_eq!(login(&app, NEW_PASSWORD).await.status, StatusCode::OK);
    let old = login(&app, DEFAULT_PASSWORD).await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
    assert_eq!(old.json(), json!({ "error": "Email ou senha inválidos" }));
}

#[tokio::test]
async fn a_nova_senha_pode_ser_igual_a_atual_e_os_tokens_antigos_caem_do_mesmo_jeito() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = put_password(&app, &token, change(DEFAULT_PASSWORD, DEFAULT_PASSWORD)).await;

    assert_eq!(response.status, StatusCode::OK);
    assert_unauthorized(&app.get("/users/me").bearer(&token).send().await);
}

#[tokio::test]
async fn o_hash_gravado_nunca_e_a_senha_em_texto_puro() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    put_password(&app, &token, change(DEFAULT_PASSWORD, NEW_PASSWORD)).await;

    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains(NEW_PASSWORD));
}

// ---- Senha atual errada ----

#[tokio::test]
async fn senha_atual_errada_da_403_e_a_senha_e_os_tokens_continuam_valendo() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = put_password(&app, &token, change("senha-errada", NEW_PASSWORD)).await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json(), json!({ "error": "Senha incorreta" }));
    assert_eq!(me_status(&app, &token).await, StatusCode::OK);
    assert_eq!(login(&app, DEFAULT_PASSWORD).await.status, StatusCode::OK);
    assert_eq!(
        login(&app, NEW_PASSWORD).await.status,
        StatusCode::UNAUTHORIZED
    );
}

// ---- Autenticação ----

#[tokio::test]
async fn sem_token_da_401_padrao() {
    let app = TestApp::new().await;

    let response = app
        .put("/users/me/password")
        .json(&change("x", NEW_PASSWORD))
        .send()
        .await;

    assert_unauthorized(&response);
}

#[tokio::test]
async fn token_revogado_por_logout_da_401_padrao() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    app.post("/auth/logout").bearer(&token).send().await;

    let response = put_password(&app, &token, change(DEFAULT_PASSWORD, NEW_PASSWORD)).await;

    assert_unauthorized(&response);
}

#[tokio::test]
async fn sem_token_e_corpo_invalido_da_401_antes_do_400() {
    let app = TestApp::new().await;

    let response = app
        .put("/users/me/password")
        .json(&json!({ "currentPassword": "" }))
        .send()
        .await;

    assert_unauthorized(&response);
}

// ---- Validação ----

#[tokio::test]
async fn current_password_invalida_da_400_de_campo_obrigatorio() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for (case, body) in [
        ("ausente", json!({ "newPassword": NEW_PASSWORD })),
        ("vazia", change("", NEW_PASSWORD)),
        (
            "null",
            json!({ "currentPassword": null, "newPassword": NEW_PASSWORD }),
        ),
        (
            "número",
            json!({ "currentPassword": 123, "newPassword": NEW_PASSWORD }),
        ),
    ] {
        let response = put_password(&app, &token, body).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": REQUIRED }), "{case}");
    }
}

#[tokio::test]
async fn new_password_invalida_da_400() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for (case, body, message) in [
        (
            "ausente",
            json!({ "currentPassword": DEFAULT_PASSWORD }),
            REQUIRED,
        ),
        (
            "null",
            json!({ "currentPassword": DEFAULT_PASSWORD, "newPassword": null }),
            REQUIRED,
        ),
        (
            "número",
            json!({ "currentPassword": DEFAULT_PASSWORD, "newPassword": 12345678 }),
            REQUIRED,
        ),
        (
            "com 7 caracteres",
            change(DEFAULT_PASSWORD, "1234567"),
            SHORT,
        ),
    ] {
        let response = put_password(&app, &token, body).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": message }), "{case}");
    }
}

#[tokio::test]
async fn os_dois_invalidos_a_mensagem_e_a_do_current_password_e_a_senha_nao_muda() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = put_password(&app, &token, change("", "123")).await;

    assert_bad_request(&response, REQUIRED);
    assert_eq!(login(&app, DEFAULT_PASSWORD).await.status, StatusCode::OK);
}

#[tokio::test]
async fn new_password_curta_com_current_password_errada_da_400_a_validacao_vem_antes_da_senha() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = put_password(&app, &token, change("errada", "123")).await;

    assert_bad_request(&response, SHORT);
}

#[tokio::test]
async fn corpo_json_que_nao_e_objeto_da_400_de_corpo_invalido() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for raw in ["null", "[]", r#""senha""#] {
        let response = app
            .put("/users/me/password")
            .bearer(&token)
            .header("content-type", "application/json")
            .raw_body(raw)
            .send()
            .await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{raw}");
        assert_eq!(response.json(), json!({ "error": INVALID_BODY }), "{raw}");
    }
}

#[tokio::test]
async fn campos_a_mais_sao_ignorados() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = put_password(
        &app,
        &token,
        json!({
            "currentPassword": DEFAULT_PASSWORD,
            "newPassword": NEW_PASSWORD,
            "role": "admin",
            "tokenVersion": 99,
        }),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
}
