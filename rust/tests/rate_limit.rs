// Rate limit (equivalente ao test/rate-limit.test.ts, que fica de fora da paridade HTTP: os contadores vivem no
// processo e o IP não pode ser trocado por HTTP). Cada teste monta um app novo, com contadores zerados.
use std::net::SocketAddr;

use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header},
    response::Response,
};
use meu_backend::{app::build_app, state::AppState};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

const SECRET: &str = "segredo-de-teste-com-pelo-menos-32-caracteres";

async fn test_pool() -> PgPool {
    dotenvy::from_filename_override("../.env.test").expect(".env.test não encontrado");
    PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap()
}

// Uma requisição "vinda" de um IP: o ConnectInfo é o que o servidor real coloca em cada requisição.
fn request(method: Method, uri: &str, ip: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .extension(ConnectInfo(
            format!("{ip}:5000").parse::<SocketAddr>().unwrap(),
        ));
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

// `app.clone()`: o Router é barato de clonar e os clones dividem o mesmo estado (e os mesmos contadores).
async fn send(app: &Router, request: Request<Body>) -> Response {
    app.clone().oneshot(request).await.unwrap()
}

fn assert_429(response: &Response) {
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after: u64 = response.headers()[header::RETRY_AFTER]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        (1..=60).contains(&retry_after),
        "Retry-After = {retry_after}"
    );
}

// Usuário criado direto no banco, com um token válido (para as rotas de /users/me).
async fn user_with_token(pool: &PgPool, state: &AppState) -> String {
    let name = format!("rl_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, 'x') RETURNING id",
    )
    .bind(&name)
    .bind(format!("{name}@email.com"))
    .fetch_one(pool)
    .await
    .unwrap();
    state.tokens.create_access_token(id, 0).unwrap()
}

#[tokio::test]
async fn login_libera_5_e_bloqueia_a_6a_inclusive_com_corpo_invalido_e_por_ip() {
    let app = build_app(AppState::new(test_pool().await, SECRET));
    let login = |ip| request(Method::POST, "/auth/login", ip, None, "{ruim");

    // Corpo inválido conta: o limite vem antes do corpo.
    for _ in 0..5 {
        assert_eq!(
            send(&app, login("10.0.0.1")).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_429(&send(&app, login("10.0.0.1")).await);
    // Outro IP tem o próprio contador.
    assert_eq!(
        send(&app, login("10.0.0.2")).await.status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn cadastro_libera_3_e_bloqueia_o_4o() {
    let app = build_app(AppState::new(test_pool().await, SECRET));

    for _ in 0..3 {
        let response = send(
            &app,
            request(Method::POST, "/auth/register", "10.0.0.3", None, "{}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert_429(
        &send(
            &app,
            request(Method::POST, "/auth/register", "10.0.0.3", None, "{}"),
        )
        .await,
    );
}

#[tokio::test]
async fn users_me_sem_token_da_401_e_nao_consome_o_limite() {
    let pool = test_pool().await;
    let state = AppState::new(pool.clone(), SECRET);
    let token = user_with_token(&pool, &state).await;
    let app = build_app(state);

    for _ in 0..6 {
        let response = send(
            &app,
            request(Method::DELETE, "/users/me", "10.0.0.4", None, "{}"),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    // Com token, a primeira tentativa ainda passa pelo limite (400: falta a senha).
    let response = send(
        &app,
        request(Method::DELETE, "/users/me", "10.0.0.4", Some(&token), "{}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_e_troca_de_senha_5_patch_10_com_contadores_separados() {
    let pool = test_pool().await;
    let state = AppState::new(pool.clone(), SECRET);
    let token = user_with_token(&pool, &state).await;
    let app = build_app(state);
    let call = |method, uri| request(method, uri, "10.0.0.5", Some(&token), "{}");

    for _ in 0..5 {
        assert_eq!(
            send(&app, call(Method::DELETE, "/users/me")).await.status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            send(&app, call(Method::PUT, "/users/me/password"))
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_429(&send(&app, call(Method::DELETE, "/users/me")).await);
    assert_429(&send(&app, call(Method::PUT, "/users/me/password")).await);

    // O PATCH tem o próprio contador, de 10.
    for _ in 0..10 {
        assert_eq!(
            send(&app, call(Method::PATCH, "/users/me")).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_429(&send(&app, call(Method::PATCH, "/users/me")).await);
}
