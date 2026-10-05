// GET /users/me (equivalente ao test/users-me.test.ts): o caminho feliz e cada jeito de um token não valer.
mod common;

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use common::{TestApp, assert_unauthorized, default_user, test_env};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::{Value, json};

async fn me(app: &TestApp, authorization: &str) -> common::TestResponse {
    app.get("/users/me")
        .header("authorization", authorization)
        .send()
        .await
}

// Segundos desde 1970, o formato das datas do JWT (iat e exp).
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

// Assina só as claims passadas: cada teste mostra exatamente o que o token tem (ou não tem).
fn sign(claims: Value, secret: &str) -> String {
    encode(
        &Header::default(), // HS256
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

fn sign_with_test_secret(claims: Value) -> String {
    sign(claims, &test_env().1)
}

// base64url sem `=` no fim (o formato das partes de um JWT). Escrito aqui para não trazer uma crate só para os
// testes: cada 3 bytes viram 4 caracteres de 6 bits.
fn base64url(value: &Value) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = value.to_string().into_bytes();
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let block = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        // 1 byte → 2 caracteres, 2 bytes → 3, 3 bytes → 4.
        for i in 0..=chunk.len() {
            text.push(ALPHABET[(block >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    text
}

async fn registered_id(app: &TestApp) -> String {
    app.register(default_user()).await["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn responde_200_com_os_dados_do_usuario_dono_do_token() {
    let app = TestApp::new().await;
    let id = registered_id(&app).await;
    let token = app
        .login(common::DEFAULT_EMAIL, common::DEFAULT_PASSWORD)
        .await;

    let response = me(&app, &format!("Bearer {token}")).await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert!(body["createdAt"].is_string());
    assert_eq!(
        body,
        json!({
            "id": id,
            "username": "joao",
            "email": "joao@email.com",
            "role": "user",
            "createdAt": body["createdAt"],
        })
    );
}

#[tokio::test]
async fn aceita_o_esquema_bearer_em_minusculas() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    assert_eq!(
        me(&app, &format!("bearer {token}")).await.status,
        StatusCode::OK
    );
}

#[tokio::test]
async fn devolve_a_role_que_esta_no_banco() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    sqlx::query("UPDATE users SET role = 'admin'")
        .execute(&app.pool)
        .await
        .unwrap();

    assert_eq!(
        me(&app, &format!("Bearer {token}")).await.json()["role"],
        "admin"
    );
}

#[tokio::test]
async fn responde_401_sem_o_header_authorization() {
    let app = TestApp::new().await;

    assert_unauthorized(&app.get("/users/me").send().await);
}

#[tokio::test]
async fn responde_401_com_authorization_invalido() {
    let app = TestApp::new().await;
    let cases = [
        ("esquema Basic", "Basic am9hbzpzZW5oYTEyMw=="),
        ("só a palavra Bearer", "Bearer"),
        ("Bearer sem token", "Bearer "),
        ("token que não é JWT", "Bearer nao-e-um-jwt"),
    ];

    for (case, authorization) in cases {
        let response = me(&app, authorization).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED, "{case}");
        assert_unauthorized(&response);
    }
}

#[tokio::test]
async fn responde_401_quando_o_payload_do_token_foi_alterado() {
    // Arrange: troca o payload (role "admin") e mantém o cabeçalho e a assinatura originais.
    let app = TestApp::new().await;
    let id = registered_id(&app).await;
    let token = app
        .login(common::DEFAULT_EMAIL, common::DEFAULT_PASSWORD)
        .await;
    let parts: Vec<&str> = token.split('.').collect();
    let forged = base64url(&json!({ "sub": id, "ver": 0, "role": "admin", "exp": now() + 3600 }));

    let response = me(&app, &format!("Bearer {}.{forged}.{}", parts[0], parts[2])).await;

    assert_unauthorized(&response);
}

#[tokio::test]
async fn responde_401_quando_o_token_foi_assinado_com_outro_segredo() {
    let app = TestApp::new().await;
    let id = registered_id(&app).await;
    let token = sign(
        json!({ "sub": id, "ver": 0, "exp": now() + 3600 }),
        "outro-segredo-qualquer-com-mais-de-32-caracteres",
    );

    assert_unauthorized(&me(&app, &format!("Bearer {token}")).await);
}

#[tokio::test]
async fn responde_401_com_token_sem_assinatura_alg_none() {
    let app = TestApp::new().await;
    let id = registered_id(&app).await;
    let header = base64url(&json!({ "alg": "none", "typ": "JWT" }));
    let payload = base64url(&json!({ "sub": id, "ver": 0, "role": "admin", "exp": now() + 3600 }));

    assert_unauthorized(&me(&app, &format!("Bearer {header}.{payload}.")).await);
}

#[tokio::test]
async fn responde_401_com_token_expirado() {
    // Arrange: segredo certo, mas vencido há 1 minuto.
    let app = TestApp::new().await;
    let id = registered_id(&app).await;
    let token = sign_with_test_secret(
        json!({ "sub": id, "ver": 0, "iat": now() - 3660, "exp": now() - 60 }),
    );

    assert_unauthorized(&me(&app, &format!("Bearer {token}")).await);
}

#[tokio::test]
async fn responde_401_com_token_sem_expiracao() {
    let app = TestApp::new().await;
    let id = registered_id(&app).await;
    let token = sign_with_test_secret(json!({ "sub": id, "ver": 0 }));

    assert_unauthorized(&me(&app, &format!("Bearer {token}")).await);
}

#[tokio::test]
async fn responde_401_quando_o_sub_do_token_nao_e_um_id_valido() {
    let app = TestApp::new().await;
    let token =
        sign_with_test_secret(json!({ "sub": "nao-e-uuid", "ver": 0, "exp": now() + 3600 }));

    assert_unauthorized(&me(&app, &format!("Bearer {token}")).await);
}

#[tokio::test]
async fn responde_401_quando_a_versao_do_token_nao_e_a_atual() {
    // Arrange: o token tem ver 0; o banco passa a exigir a versão 1 (como faz o logout).
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    sqlx::query("UPDATE users SET token_version = 1")
        .execute(&app.pool)
        .await
        .unwrap();

    assert_unauthorized(&me(&app, &format!("Bearer {token}")).await);
}

#[tokio::test]
async fn responde_401_com_token_valido_e_bem_assinado_mas_sem_ver() {
    let app = TestApp::new().await;
    let id = registered_id(&app).await;
    let token = sign_with_test_secret(json!({ "sub": id, "exp": now() + 3600 }));

    assert_unauthorized(&me(&app, &format!("Bearer {token}")).await);
}

#[tokio::test]
async fn responde_401_quando_o_usuario_foi_apagado_depois_do_login() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    sqlx::query("DELETE FROM users")
        .execute(&app.pool)
        .await
        .unwrap();

    assert_unauthorized(&me(&app, &format!("Bearer {token}")).await);
}

#[tokio::test]
async fn get_auth_me_nao_existe_mais_404() {
    // Um token válido, para provar que o 404 é da rota e não de falta de autenticação.
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = app.get("/auth/me").bearer(&token).send().await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

// O base64url acima precisa estar certo para os testes de token forjado valerem alguma coisa.
#[test]
fn base64url_confere_com_o_padrao() {
    assert_eq!(base64url(&json!("a")), "ImEi"); // "a" com aspas: 3 bytes
    assert_eq!(base64url(&json!(1)), "MQ"); // 1 byte
    assert_eq!(base64url(&json!(12)), "MTI"); // 2 bytes
}
