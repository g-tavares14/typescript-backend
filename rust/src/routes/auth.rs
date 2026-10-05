// Rotas de /auth (o equivalente ao src/routes/auth.ts). Por enquanto, só o cadastro.
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::AppError,
    json::JsonBody,
    password::hash_password,
    user_fields::{DUPLICATE_USER, check_new_password, parse_email, parse_username},
    validation::required_string,
};

pub fn router() -> Router<PgPool> {
    Router::new().route("/register", post(register))
}

// Resposta do cadastro. `#[derive(Serialize)]` gera a conversão para JSON (o serde lê os nomes dos campos).
#[derive(Serialize)]
struct RegisteredUser {
    id: Uuid,
    username: String,
}

// Os extractors vêm na ordem dos parâmetros; o corpo (JsonBody) precisa ser o último, porque consome a requisição.
async fn register(
    State(pool): State<PgPool>,
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
