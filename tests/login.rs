// POST /auth/login (equivalente ao test/login.test.ts).
mod common;

use axum::http::StatusCode;
use common::{DEFAULT_EMAIL, DEFAULT_PASSWORD, TestApp, default_user, test_env};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde_json::{Value, json};

fn credentials(email: &str, password: &str) -> Value {
    json!({ "email": email, "password": password })
}

#[tokio::test]
async fn responde_200_com_um_jwt_de_1_hora_para_o_usuario() {
    let app = TestApp::new().await;
    let id = app.register(default_user()).await["id"].clone();

    let response = app
        .post("/auth/login")
        .json(&credentials(DEFAULT_EMAIL, DEFAULT_PASSWORD))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert!(body["token"].is_string());
    assert_eq!(body["tokenType"], "Bearer");
    assert_eq!(body["expiresIn"], 3600);
    assert_eq!(body.as_object().unwrap().len(), 3);

    // Lê as claims conferindo a assinatura com o segredo dos testes (o decodeJwt do TS só lia, sem conferir).
    let (_, secret) = test_env();
    let claims = decode::<Value>(
        body["token"].as_str().unwrap(),
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .unwrap()
    .claims;
    assert_eq!(claims["sub"], id);
    assert_eq!(claims["ver"], 0);
    assert_eq!(
        claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(),
        3600
    );
    // A role não vai no token: quem precisa dela consulta GET /users/me. E nada de senha.
    for field in ["role", "password", "passwordHash"] {
        assert!(claims.get(field).is_none(), "o token não pode ter {field}");
    }
}

#[tokio::test]
async fn aceita_o_email_com_maiusculas_e_espacos() {
    let app = TestApp::new().await;
    app.register(default_user()).await;

    let response = app
        .post("/auth/login")
        .json(&credentials("  JOAO@Email.COM ", DEFAULT_PASSWORD))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::OK);
}

#[tokio::test]
async fn responde_401_quando_a_senha_esta_errada() {
    let app = TestApp::new().await;
    app.register(default_user()).await;

    let response = app
        .post("/auth/login")
        .json(&credentials(DEFAULT_EMAIL, "senha-errada"))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.json(),
        json!({ "error": "Email ou senha inválidos" })
    );
}

#[tokio::test]
async fn responde_401_com_a_mesma_mensagem_quando_o_email_nao_existe() {
    let app = TestApp::new().await;
    app.register(default_user()).await;
    let wrong_password = app
        .post("/auth/login")
        .json(&credentials(DEFAULT_EMAIL, "senha-errada"))
        .send()
        .await;

    let unknown_email = app
        .post("/auth/login")
        .json(&credentials("ninguem@email.com", "senha-errada"))
        .send()
        .await;

    // A resposta não pode revelar se o email tem conta.
    assert_eq!(unknown_email.status, wrong_password.status);
    assert_eq!(unknown_email.body, wrong_password.body);
}

#[tokio::test]
async fn a_senha_diferencia_maiusculas_de_minusculas() {
    let app = TestApp::new().await;
    app.register(default_user()).await;

    let response = app
        .post("/auth/login")
        .json(&credentials(
            DEFAULT_EMAIL,
            &DEFAULT_PASSWORD.to_uppercase(),
        ))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn responde_400_com_cada_corpo_invalido() {
    let app = TestApp::new().await;
    let cases = [
        (
            "corpo vazio",
            json!({}),
            "Campo obrigatório ausente ou inválido",
        ),
        (
            "senha vazia",
            credentials("joao@email.com", ""),
            "Campo obrigatório ausente ou inválido",
        ),
        (
            "email inválido",
            credentials("nao-e-email", "senha123"),
            "Email inválido",
        ),
    ];

    for (case, body, message) in cases {
        let response = app.post("/auth/login").json(&body).send().await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": message }), "{case}");
    }
}
