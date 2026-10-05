// POST /auth/register (equivalente ao test/register.test.ts).
mod common;

use axum::http::StatusCode;
use common::{TestApp, assert_bad_request, default_user};
use serde_json::{Value, json};
use uuid::Uuid;

const USERNAME_FORMAT_ERROR: &str = "O username só pode ter letras sem acento, números e _";
const DUPLICATE: &str = "Email ou username já cadastrado";

// O usuário padrão com alguns campos trocados (o `{ ...defaultUser, campo: valor }` do TS).
fn user_with(changes: Value) -> Value {
    let mut user = default_user();
    for (key, value) in changes.as_object().unwrap() {
        user[key] = value.clone();
    }
    user
}

#[tokio::test]
async fn cria_o_usuario_e_responde_201() {
    let app = TestApp::new().await;

    let response = app
        .post("/auth/register")
        .json(&default_user())
        .send()
        .await;

    assert_eq!(response.status, StatusCode::CREATED);
    let body = response.json();
    assert!(Uuid::parse_str(body["id"].as_str().unwrap()).is_ok());
    assert_eq!(body, json!({ "id": body["id"], "username": "joao" }));
}

#[tokio::test]
async fn responde_409_quando_o_email_ja_esta_cadastrado() {
    let app = TestApp::new().await;
    app.register(default_user()).await;

    let response = app
        .post("/auth/register")
        .json(&user_with(json!({ "username": "outro_nome" })))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json(), json!({ "error": DUPLICATE }));
}

#[tokio::test]
async fn responde_400_quando_a_senha_tem_menos_de_8_caracteres() {
    let app = TestApp::new().await;

    let response = app
        .post("/auth/register")
        .json(&user_with(json!({ "password": "123" })))
        .send()
        .await;

    assert_bad_request(&response, "A senha deve ter no mínimo 8 caracteres");
}

#[tokio::test]
async fn responde_409_quando_o_username_ja_esta_cadastrado() {
    let app = TestApp::new().await;
    app.register(default_user()).await;

    let response = app
        .post("/auth/register")
        .json(&user_with(json!({ "email": "outro@email.com" })))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json(), json!({ "error": DUPLICATE }));
}

#[tokio::test]
async fn normaliza_o_email_mesmo_email_com_maiusculas_e_espacos_e_duplicado() {
    let app = TestApp::new().await;
    app.register(default_user()).await;

    let response = app
        .post("/auth/register")
        .json(&user_with(
            json!({ "username": "outro_nome", "email": "  JOAO@Email.com " }),
        ))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn normaliza_o_username_salva_e_devolve_em_minusculas() {
    let app = TestApp::new().await;

    let response = app
        .post("/auth/register")
        .json(&user_with(json!({ "username": "Joao" })))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::CREATED);
    assert_eq!(response.json()["username"], "joao");
}

#[tokio::test]
async fn responde_409_quando_so_as_maiusculas_do_username_mudam() {
    let app = TestApp::new().await;
    app.register(default_user()).await;

    let response = app
        .post("/auth/register")
        .json(&user_with(
            json!({ "username": "Joao", "email": "outro@email.com" }),
        ))
        .send()
        .await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json(), json!({ "error": DUPLICATE }));
}

#[tokio::test]
async fn responde_400_com_cada_campo_invalido() {
    let app = TestApp::new().await;
    let cases = [
        (
            "corpo vazio",
            json!({}),
            "Campo obrigatório ausente ou inválido",
        ),
        (
            "email inválido",
            user_with(json!({ "email": "nao-e-email" })),
            "Email inválido",
        ),
        (
            "username curto",
            user_with(json!({ "username": "jo" })),
            "O username deve ter entre 3 e 50 caracteres",
        ),
        (
            "username com acento",
            user_with(json!({ "username": "joão" })),
            USERNAME_FORMAT_ERROR,
        ),
        (
            "username com espaço",
            user_with(json!({ "username": "jo ao" })),
            USERNAME_FORMAT_ERROR,
        ),
        // "о" abaixo é a letra cirílica U+043E, visualmente igual ao "o" latino.
        (
            "username com letra cirílica parecida com latina",
            user_with(json!({ "username": "j\u{043e}ao" })),
            USERNAME_FORMAT_ERROR,
        ),
        (
            "username com caractere invisível (zero-width space)",
            user_with(json!({ "username": "joao\u{200b}" })),
            USERNAME_FORMAT_ERROR,
        ),
        (
            "username com hífen",
            user_with(json!({ "username": "joao-silva" })),
            USERNAME_FORMAT_ERROR,
        ),
        (
            "campo com tipo errado",
            user_with(json!({ "password": 12345678 })),
            "Campo obrigatório ausente ou inválido",
        ),
    ];

    // Um laço com o nome do caso na mensagem de falha: o `test.each` do vitest.
    for (case, body, message) in cases {
        let response = app.post("/auth/register").json(&body).send().await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": message }), "{case}");
    }
}

#[tokio::test]
async fn salva_so_o_hash_argon2id_da_senha_nunca_a_senha_em_texto_puro() {
    let app = TestApp::new().await;
    let id = app.register(default_user()).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1::uuid")
        .bind(&id)
        .fetch_one(&app.pool)
        .await
        .unwrap();

    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains("senha123"));
}

#[tokio::test]
async fn ignora_a_role_enviada_na_requisicao_todo_cadastro_nasce_como_user() {
    let app = TestApp::new().await;
    let id = app.register(user_with(json!({ "role": "admin" }))).await["id"]
        .as_str()
        .unwrap()
        .to_string();

    let role: String = sqlx::query_scalar("SELECT role FROM users WHERE id = $1::uuid")
        .bind(&id)
        .fetch_one(&app.pool)
        .await
        .unwrap();

    assert_eq!(role, "user");
}
