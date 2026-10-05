// Rate limit (equivalente ao test/rate-limit.test.ts). Cada teste monta um app novo, com contadores zerados, e
// escolhe o IP de cada requisição com `.ip(...)`.
mod common;

use axum::http::{StatusCode, header};
use common::{TestApp, TestResponse};
use uuid::Uuid;

fn assert_429(response: &TestResponse) {
    assert_eq!(response.status, StatusCode::TOO_MANY_REQUESTS);
    let retry_after: u64 = response
        .header(header::RETRY_AFTER)
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (1..=60).contains(&retry_after),
        "Retry-After = {retry_after}"
    );
}

// Usuário criado direto no banco, com um token válido (para as rotas de /users/me), sem passar pelo cadastro,
// que também tem limite.
async fn user_with_token(app: &TestApp) -> String {
    let name = format!("rl_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, 'x') RETURNING id",
    )
    .bind(&name)
    .bind(format!("{name}@email.com"))
    .fetch_one(&app.pool)
    .await
    .unwrap();
    app.state.tokens.create_access_token(id, 0).unwrap()
}

#[tokio::test]
async fn login_libera_5_e_bloqueia_a_6a_inclusive_com_corpo_invalido_e_por_ip() {
    let app = TestApp::with_rate_limit().await;
    let login = |ip| {
        app.post("/auth/login")
            .header("content-type", "application/json")
            .raw_body("{ruim")
            .ip(ip)
    };

    // Corpo inválido conta: o limite vem antes do corpo.
    for _ in 0..5 {
        assert_eq!(
            login("10.0.0.1").send().await.status,
            StatusCode::BAD_REQUEST
        );
    }
    assert_429(&login("10.0.0.1").send().await);
    // Outro IP tem o próprio contador.
    assert_eq!(
        login("10.0.0.2").send().await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn cadastro_libera_3_e_bloqueia_o_4o() {
    let app = TestApp::with_rate_limit().await;
    let register = || {
        app.post("/auth/register")
            .json(&serde_json::json!({}))
            .ip("10.0.0.3")
    };

    for _ in 0..3 {
        assert_eq!(register().send().await.status, StatusCode::BAD_REQUEST);
    }
    assert_429(&register().send().await);
}

#[tokio::test]
async fn users_me_sem_token_da_401_e_nao_consome_o_limite() {
    let app = TestApp::with_rate_limit().await;
    let token = user_with_token(&app).await;
    let delete = || {
        app.delete("/users/me")
            .json(&serde_json::json!({}))
            .ip("10.0.0.4")
    };

    for _ in 0..6 {
        assert_eq!(delete().send().await.status, StatusCode::UNAUTHORIZED);
    }
    // Com token, a primeira tentativa ainda passa pelo limite (400: falta a senha).
    assert_eq!(
        delete().bearer(&token).send().await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn delete_e_troca_de_senha_5_patch_10_com_contadores_separados() {
    let app = TestApp::with_rate_limit().await;
    let token = user_with_token(&app).await;
    let empty = serde_json::json!({});
    let delete = || {
        app.delete("/users/me")
            .json(&empty)
            .bearer(&token)
            .ip("10.0.0.5")
    };
    let password = || {
        app.put("/users/me/password")
            .json(&empty)
            .bearer(&token)
            .ip("10.0.0.5")
    };
    let patch = || {
        app.patch("/users/me")
            .json(&empty)
            .bearer(&token)
            .ip("10.0.0.5")
    };

    for _ in 0..5 {
        assert_eq!(delete().send().await.status, StatusCode::BAD_REQUEST);
        assert_eq!(password().send().await.status, StatusCode::BAD_REQUEST);
    }
    assert_429(&delete().send().await);
    assert_429(&password().send().await);

    // O PATCH tem o próprio contador, de 10.
    for _ in 0..10 {
        assert_eq!(patch().send().await.status, StatusCode::BAD_REQUEST);
    }
    assert_429(&patch().send().await);
}
