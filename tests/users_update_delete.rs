// PATCH e DELETE /users/me (equivalente ao test/users-update-delete.test.ts). As corridas (conta apagada no meio do
// caminho) ficam em tests/users.rs.
mod common;

use axum::http::StatusCode;
use common::{
    DEFAULT_EMAIL, DEFAULT_PASSWORD, TestApp, TestResponse, assert_bad_request, assert_unauthorized,
};
use serde_json::{Value, json};

const REQUIRED: &str = "Campo obrigatório ausente ou inválido";
const NO_FIELDS: &str = "Envie ao menos um campo para alterar";
const INVALID_BODY: &str = "Corpo da requisição inválido: envie um objeto JSON";
const USERNAME_LENGTH: &str = "O username deve ter entre 3 e 50 caracteres";
const USERNAME_CHARS: &str = "O username só pode ter letras sem acento, números e _";
const CONFLICT: &str = "Email ou username já cadastrado";
const WRONG_PASSWORD: &str = "Senha incorreta";
const UNSUPPORTED: &str = "Tipo de conteúdo não suportado (use application/json)";

fn maria() -> Value {
    json!({ "username": "maria", "email": "maria@email.com", "password": "senha123" })
}

async fn patch_me(app: &TestApp, token: &str, body: Value) -> TestResponse {
    app.patch("/users/me")
        .bearer(token)
        .json(&body)
        .send()
        .await
}

async fn me(app: &TestApp, token: &str) -> Value {
    let response = app.get("/users/me").bearer(token).send().await;
    assert_eq!(response.status, StatusCode::OK);
    response.json()
}

// Confere username e email da conta (o `toMatchObject` dos testes do TS).
async fn assert_profile(app: &TestApp, token: &str, username: &str, email: &str) {
    let body = me(app, token).await;
    assert_eq!(
        (body["username"].as_str(), body["email"].as_str()),
        (Some(username), Some(email))
    );
}

async fn login_status(app: &TestApp, email: &str, password: &str) -> StatusCode {
    app.post("/auth/login")
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .status
}

// ---- PATCH /users/me ----

#[tokio::test]
async fn patch_altera_so_o_username_normalizado_e_o_mesmo_token_continua_valendo() {
    let app = TestApp::new().await;
    let id = app.register(common::default_user()).await["id"].clone();
    let token = app.login(DEFAULT_EMAIL, DEFAULT_PASSWORD).await;

    let response = patch_me(&app, &token, json!({ "username": "  Joao_Silva " })).await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert!(body["createdAt"].is_string());
    assert_eq!(
        body,
        json!({
            "id": id,
            "username": "joao_silva",
            "email": "joao@email.com",
            "role": "user",
            "createdAt": body["createdAt"],
        })
    );
    assert_profile(&app, &token, "joao_silva", "joao@email.com").await;
}

#[tokio::test]
async fn patch_altera_so_o_email_normalizado_sem_mexer_no_username() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = patch_me(&app, &token, json!({ "email": "  Novo@Email.COM " })).await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["email"], "novo@email.com");
    assert_profile(&app, &token, "joao", "novo@email.com").await;
}

#[tokio::test]
async fn patch_altera_os_dois_campos_de_uma_vez() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = patch_me(
        &app,
        &token,
        json!({ "username": "novo_nome", "email": "novo@email.com" }),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert_eq!(
        (&body["username"], &body["email"]),
        (&json!("novo_nome"), &json!("novo@email.com"))
    );
}

#[tokio::test]
async fn depois_de_trocar_o_email_o_login_com_o_novo_funciona_e_com_o_antigo_da_401() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    patch_me(&app, &token, json!({ "email": "novo@email.com" })).await;

    assert_eq!(
        login_status(&app, "novo@email.com", DEFAULT_PASSWORD).await,
        StatusCode::OK
    );
    let old = app
        .post("/auth/login")
        .json(&json!({ "email": DEFAULT_EMAIL, "password": DEFAULT_PASSWORD }))
        .send()
        .await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
    assert_eq!(old.json(), json!({ "error": "Email ou senha inválidos" }));
}

#[tokio::test]
async fn patch_com_o_proprio_valor_atual_responde_200() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = patch_me(
        &app,
        &token,
        json!({ "username": "joao", "email": "joao@email.com" }),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_profile(&app, &token, "joao", "joao@email.com").await;
}

#[tokio::test]
async fn patch_com_username_de_outra_conta_da_409_mesmo_em_maiusculas() {
    let app = TestApp::new().await;
    app.register(maria()).await;
    let token = app.register_and_login().await;

    let response = patch_me(&app, &token, json!({ "username": "MARIA" })).await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json(), json!({ "error": CONFLICT }));
}

#[tokio::test]
async fn patch_com_email_de_outra_conta_da_409_e_o_username_enviado_junto_nao_muda() {
    let app = TestApp::new().await;
    app.register(maria()).await;
    let token = app.register_and_login().await;

    let response = patch_me(
        &app,
        &token,
        json!({ "username": "outro_nome", "email": "maria@email.com" }),
    )
    .await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    assert_eq!(response.json(), json!({ "error": CONFLICT }));
    assert_profile(&app, &token, "joao", "joao@email.com").await;
}

#[tokio::test]
async fn patch_nao_altera_a_conta_de_outro_usuario() {
    let app = TestApp::new().await;
    let other_id = app.register(maria()).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let token = app.register_and_login().await;

    patch_me(
        &app,
        &token,
        json!({ "username": "novo_nome", "email": "novo@email.com" }),
    )
    .await;

    let other: (String, String) =
        sqlx::query_as("SELECT username, email FROM users WHERE id = $1::uuid")
            .bind(&other_id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(other, ("maria".to_string(), "maria@email.com".to_string()));
}

#[tokio::test]
async fn patch_responde_401_sem_token_e_com_token_revogado_por_logout() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let body = json!({ "username": "novo_nome" });

    assert_unauthorized(&app.patch("/users/me").json(&body).send().await);

    app.post("/auth/logout").bearer(&token).send().await;
    assert_unauthorized(&patch_me(&app, &token, body).await);
}

// ---- PATCH /users/me: validação ----

#[tokio::test]
async fn patch_sem_campo_editavel_da_400_e_nada_muda() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for (case, body) in [
        ("{}", json!({})),
        ("só campo desconhecido", json!({ "foo": 1 })),
        ("só password", json!({ "password": "x" })),
    ] {
        let response = patch_me(&app, &token, body).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": NO_FIELDS }), "{case}");
    }
    assert_profile(&app, &token, "joao", "joao@email.com").await;
}

#[tokio::test]
async fn patch_com_null_ou_tipo_errado_da_400_de_campo_obrigatorio() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for body in [
        json!({ "email": null }),
        json!({ "username": null }),
        json!({ "username": 123 }),
        json!({ "email": 123 }),
    ] {
        let response = patch_me(&app, &token, body.clone()).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(response.json(), json!({ "error": REQUIRED }), "{body}");
    }
}

#[tokio::test]
async fn patch_com_username_invalido_da_400_com_a_mensagem_do_username_e_nada_muda() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let long = "a".repeat(51);

    for (case, username, message) in [
        ("curto", "ab", USERNAME_LENGTH),
        ("longo", long.as_str(), USERNAME_LENGTH),
        ("com acento", "joão", USERNAME_CHARS),
        ("com espaço no meio", "joao silva", USERNAME_CHARS),
    ] {
        let response = patch_me(&app, &token, json!({ "username": username })).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": message }), "{case}");
    }
    assert_profile(&app, &token, "joao", "joao@email.com").await;
}

#[tokio::test]
async fn patch_com_email_invalido_da_400() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = patch_me(&app, &token, json!({ "email": "nao-e-email" })).await;

    assert_bad_request(&response, "Email inválido");
}

#[tokio::test]
async fn patch_com_um_campo_valido_e_outro_invalido_nao_grava_nem_o_valido() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = patch_me(
        &app,
        &token,
        json!({ "email": "novo@email.com", "username": "ab" }),
    )
    .await;

    assert_bad_request(&response, USERNAME_LENGTH);
    assert_profile(&app, &token, "joao", "joao@email.com").await;
}

#[tokio::test]
async fn patch_com_corpo_json_que_nao_e_objeto_da_400_de_corpo_invalido() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for raw in ["null", "[]", r#""oi""#] {
        let response = app
            .patch("/users/me")
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
async fn patch_ignora_role_e_password_no_corpo() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = patch_me(
        &app,
        &token,
        json!({ "username": "novo_nome", "role": "admin", "password": "outrasenha1" }),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["role"], "user");
    let role: String = sqlx::query_scalar("SELECT role FROM users")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(role, "user");
    // A senha antiga continua entrando.
    assert_eq!(
        login_status(&app, DEFAULT_EMAIL, DEFAULT_PASSWORD).await,
        StatusCode::OK
    );
}

// ---- DELETE /users/me ----

async fn delete_me(app: &TestApp, token: &str, body: Value) -> TestResponse {
    app.delete("/users/me")
        .bearer(token)
        .json(&body)
        .send()
        .await
}

async fn create_transaction(app: &TestApp, token: &str) {
    let response = app
        .post("/transactions")
        .bearer(token)
        .json(&json!({ "type": "expense", "amount": 1990, "description": "Almoço", "date": "2026-09-29" }))
        .send()
        .await;
    assert_eq!(response.status, StatusCode::CREATED);
}

#[tokio::test]
async fn delete_com_a_senha_certa_da_204_e_o_token_o_login_e_a_conta_somem() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = delete_me(&app, &token, json!({ "password": DEFAULT_PASSWORD })).await;

    assert_eq!(response.status, StatusCode::NO_CONTENT);
    assert_eq!(response.body, "");
    assert_unauthorized(&app.get("/users/me").bearer(&token).send().await);
    assert_eq!(
        login_status(&app, DEFAULT_EMAIL, DEFAULT_PASSWORD).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn depois_do_delete_email_e_username_podem_ser_cadastrados_de_novo() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    delete_me(&app, &token, json!({ "password": DEFAULT_PASSWORD })).await;

    let response = app
        .post("/auth/register")
        .json(&common::default_user())
        .send()
        .await;

    assert_eq!(response.status, StatusCode::CREATED);
}

#[tokio::test]
async fn delete_apaga_os_registros_financeiros_do_usuario_e_mantem_os_de_outro() {
    let app = TestApp::new().await;
    let token_a = app.register_and_login().await;
    let token_b = app.register_and_login_as(maria()).await;
    create_transaction(&app, &token_a).await;
    create_transaction(&app, &token_b).await;

    let response = delete_me(&app, &token_a, json!({ "password": DEFAULT_PASSWORD })).await;

    assert_eq!(response.status, StatusCode::NO_CONTENT);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let list = app
        .get("/transactions")
        .bearer(&token_b)
        .send()
        .await
        .json();
    assert_eq!(list["transactions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn delete_com_senha_errada_da_403_e_a_conta_continua() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = delete_me(&app, &token, json!({ "password": "errada123" })).await;

    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json(), json!({ "error": WRONG_PASSWORD }));
    me(&app, &token).await;
    assert_eq!(
        login_status(&app, DEFAULT_EMAIL, DEFAULT_PASSWORD).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn delete_sem_senha_valida_da_400_de_campo_obrigatorio() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for body in [
        json!({}),
        json!({ "password": "" }),
        json!({ "password": 123 }),
    ] {
        let response = delete_me(&app, &token, body.clone()).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(response.json(), json!({ "error": REQUIRED }), "{body}");
    }
    me(&app, &token).await;
}

#[tokio::test]
async fn delete_com_corpo_json_que_nao_e_objeto_da_400_de_corpo_invalido() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for (case, raw) in [("sem corpo", ""), ("null", "null"), ("array", "[]")] {
        let response = app
            .delete("/users/me")
            .bearer(&token)
            .header("content-type", "application/json")
            .raw_body(raw)
            .send()
            .await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": INVALID_BODY }), "{case}");
    }
}

// Diferença para o TS ("Diferenças para o front"): sem Content-Type ou com text/plain, o axum responde 415 (o
// Fastify dava 400 de corpo inválido). A conta continua.
#[tokio::test]
async fn delete_sem_content_type_ou_com_tipo_que_nao_e_json_da_415() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for (case, content_type, raw) in [
        ("sem corpo e sem Content-Type", None, ""),
        ("text/plain", Some("text/plain"), "senha123"),
        ("application/xml", Some("application/xml"), "<a/>"),
    ] {
        let mut request = app.delete("/users/me").bearer(&token).raw_body(raw);
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        let response = request.send().await;
        assert_eq!(
            response.status,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "{case}"
        );
        assert_eq!(response.json(), json!({ "error": UNSUPPORTED }), "{case}");
    }
    me(&app, &token).await;
}

#[tokio::test]
async fn delete_responde_401_sem_token_e_com_token_revogado_por_logout() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let body = json!({ "password": DEFAULT_PASSWORD });

    assert_unauthorized(&app.delete("/users/me").json(&body).send().await);

    app.post("/auth/logout").bearer(&token).send().await;
    assert_unauthorized(&delete_me(&app, &token, body).await);
}

#[tokio::test]
async fn o_hash_da_senha_nao_aparece_em_nenhuma_resposta() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let responses = [
        app.get("/users/me").bearer(&token).send().await,
        delete_me(&app, &token, json!({ "password": "errada123" })).await,
        delete_me(&app, &token, json!({ "password": DEFAULT_PASSWORD })).await,
    ];

    for response in responses {
        for forbidden in ["passwordHash", "password_hash", "argon2"] {
            assert!(!response.body.contains(forbidden), "{}", response.body);
        }
    }
}
