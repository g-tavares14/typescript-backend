// Rotas de /auth (o equivalente ao src/routes/auth.ts): cadastro e login.
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::CurrentUser,
    error::AppError,
    json::JsonBody,
    password::{hash_password, simulate_password_verification, verify_password},
    rate_limit::{Login, RateLimited, Register},
    state::AppState,
    token::ACCESS_TOKEN_TTL_SECONDS,
    user_fields::{DUPLICATE_USER, check_new_password, parse_email, parse_username},
    validation::{REQUIRED, bad_request, required_string},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
}

// Resposta do cadastro. `#[derive(Serialize)]` gera a conversão para JSON (o serde lê os nomes dos campos).
#[derive(Serialize)]
struct RegisteredUser {
    id: Uuid,
    username: String,
}

// Os extractors vêm na ordem dos parâmetros; o corpo (JsonBody) precisa ser o último, porque consome a requisição.
// O RateLimited vem antes do corpo: corpo inválido também conta no limite.
async fn register(
    State(pool): State<PgPool>,
    _limit: RateLimited<Register>,
    JsonBody(body): JsonBody,
) -> Result<(StatusCode, Json<RegisteredUser>), AppError> {
    // Validação na ordem do schema do TS (username, email, password): o primeiro erro encontrado é a resposta.
    // Campos a mais (como `role`) nem são lidos: a role nunca vem da requisição.
    let username = parse_username(required_string(&body, "username")?)?;
    let email = parse_email(required_string(&body, "email")?)?;
    let password = required_string(&body, "password")?;
    check_new_password(password)?;

    // `.to_string()`: a senha vai para outra thread (spawn_blocking), então precisa ser uma cópia com dono próprio,
    // e não um empréstimo do corpo da requisição.
    let password_hash = hash_password(password.to_string()).await?;

    // `query_as!` confere o SQL e os tipos contra o banco NA COMPILAÇÃO (precisa do DATABASE_URL no build).
    // `$1`, `$2`, `$3` são parâmetros: os valores vão separados do SQL, nunca concatenados.
    // id, role e created_at ficam com os valores padrão do banco.
    let result = sqlx::query_as!(
        RegisteredUser,
        "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING id, username",
        username,
        email,
        password_hash,
    )
    .fetch_one(&pool)
    .await;

    match result {
        Ok(user) => Ok((StatusCode::CREATED, Json(user))),
        // 23505 = violação de UNIQUE (email ou username já usados). O banco não diz qual, e a mensagem também não.
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(AppError::Conflict(DUPLICATE_USER))
        }
        // Qualquer outro erro do banco vira 500 (o `From<sqlx::Error>` do AppError).
        Err(error) => Err(error.into()),
    }
}

// Resposta do login. `rename_all = "camelCase"`: os campos em snake_case do Rust saem em camelCase no JSON.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    token: String,
    token_type: &'static str,
    expires_in: u64,
}

async fn login(
    State(state): State<AppState>,
    _limit: RateLimited<Login>,
    JsonBody(body): JsonBody,
) -> Result<Json<LoginResponse>, AppError> {
    // Ordem do schema do TS: email (mesma regra do cadastro) e depois a senha, sem tamanho mínimo (a regra de 8 é do
    // cadastro: contas antigas com senhas menores continuariam entrando se a regra mudar).
    let email = parse_email(required_string(&body, "email")?)?;
    let password = required_string(&body, "password")?;
    if password.is_empty() {
        return Err(bad_request(REQUIRED));
    }

    // `fetch_optional`: Option<linha> (None se o email não existe), em vez de erro.
    let user = sqlx::query!(
        "SELECT id, token_version, password_hash FROM users WHERE email = $1",
        email
    )
    .fetch_optional(&state.pool)
    .await?;

    // `let ... else`: sem usuário, gasta o tempo de uma verificação e responde o mesmo 401 da senha errada.
    let Some(user) = user else {
        simulate_password_verification(password.to_string()).await?;
        return Err(AppError::InvalidCredentials);
    };
    if !verify_password(user.password_hash, password.to_string()).await? {
        return Err(AppError::InvalidCredentials);
    }

    let token = state
        .tokens
        .create_access_token(user.id, user.token_version)?;
    Ok(Json(LoginResponse {
        token,
        token_type: "Bearer",
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
    }))
}

// Logout em todos os dispositivos: subir a versão invalida todos os tokens já emitidos para o usuário.
// O CurrentUser é só desta rota: register e login continuam públicos.
async fn logout(State(pool): State<PgPool>, current: CurrentUser) -> Result<StatusCode, AppError> {
    // O incremento é feito pelo banco (token_version + 1), não lendo o valor e somando aqui: dois logouts
    // simultâneos não se atropelam e cada um conta.
    sqlx::query!(
        "UPDATE users SET token_version = token_version + 1 WHERE id = $1",
        current.user.id
    )
    .execute(&pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
