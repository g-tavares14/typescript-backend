// Rotas de /users (o equivalente ao src/routes/users.ts). Todas exigem login: cada handler pede um CurrentUser.
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, put},
};
use serde_json::{Map, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::{CurrentUser, PublicUser},
    error::AppError,
    json::JsonBody,
    password::{hash_password, verify_password},
    state::AppState,
    token::ACCESS_TOKEN_TTL_SECONDS,
    user_fields::{DUPLICATE_USER, check_new_password, parse_email, parse_username},
    validation::{REQUIRED, bad_request, required_string},
};

pub fn router() -> Router<AppState> {
    // Duas rotas no mesmo caminho: `get(me).patch(update_me)` junta os métodos.
    Router::new()
        .route("/me", get(me).patch(update_me).delete(delete_me))
        .route("/me/password", put(change_password_route))
}

// O extractor já validou o token e leu o usuário: aqui é só devolver.
async fn me(current: CurrentUser) -> Json<PublicUser> {
    Json(current.user)
}

// Campos que o PATCH /users/me pode mudar. `None` = não enviado (continua como está).
#[derive(Debug, Default, PartialEq)]
pub struct ProfileChanges {
    pub username: Option<String>,
    pub email: Option<String>,
}

// Lê um campo opcional do PATCH: ausente → Ok(None); presente → precisa ser string (null ou outro tipo dão
// REQUIRED) e passar pela regra do campo (`parse`, uma função recebida como parâmetro).
fn optional_field(
    body: &Map<String, Value>,
    field: &str,
    parse: fn(&str) -> Result<String, AppError>,
) -> Result<Option<String>, AppError> {
    match body.get(field) {
        None => Ok(None),
        Some(Value::String(text)) => parse(text).map(Some),
        Some(_) => Err(bad_request(REQUIRED)),
    }
}

// Validação na ordem do schema do TS (username, email); só depois, "ao menos um campo".
// Outros campos (role, password, id...) são ignorados: nem são lidos.
fn parse_changes(body: &Map<String, Value>) -> Result<ProfileChanges, AppError> {
    let changes = ProfileChanges {
        username: optional_field(body, "username", parse_username)?,
        email: optional_field(body, "email", parse_email)?,
    };
    if changes == ProfileChanges::default() {
        return Err(bad_request("Envie ao menos um campo para alterar"));
    }
    Ok(changes)
}

async fn update_me(
    State(pool): State<PgPool>,
    current: CurrentUser,
    JsonBody(body): JsonBody,
) -> Result<Json<PublicUser>, AppError> {
    let changes = parse_changes(&body)?;
    Ok(Json(update_profile(&pool, current.user.id, changes).await?))
}

// A gravação, separada do handler para os testes chamarem direto (ex.: conta que sumiu no meio do caminho).
// Uma consulta só; o id vem do token, nunca do corpo. `COALESCE($1, username)`: com $1 nulo (campo não enviado),
// a coluna recebe o próprio valor. O UNIQUE do banco decide o 409 (sem SELECT antes, que teria corrida).
pub async fn update_profile(
    pool: &PgPool,
    user_id: Uuid,
    changes: ProfileChanges,
) -> Result<PublicUser, AppError> {
    let result = sqlx::query_as!(
        PublicUser,
        "UPDATE users SET username = COALESCE($1, username), email = COALESCE($2, email)
         WHERE id = $3
         RETURNING id, username, email, role, created_at",
        changes.username,
        changes.email,
        user_id,
    )
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some(user)) => Ok(user),
        // 0 linhas: a conta foi apagada entre a autenticação e o UPDATE. Para o cliente, é o 401 de sempre.
        Ok(None) => Err(AppError::Unauthorized),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(AppError::Conflict(DUPLICATE_USER))
        }
        Err(error) => Err(error.into()),
    }
}

// 403 (e não 401) quando a confirmação de senha falha: o token é válido, e o 401 faria o front deslogar o usuário.
const WRONG_PASSWORD: &str = "Senha incorreta";

// Senha de confirmação: string não vazia, sem tamanho mínimo (como no login).
fn confirmation_password<'a>(
    body: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, AppError> {
    let password = required_string(body, field)?;
    if password.is_empty() {
        return Err(bad_request(REQUIRED));
    }
    Ok(password)
}

// Confere a senha atual do usuário. O hash é lido só aqui: o CurrentUser não o carrega, para ele nunca chegar
// perto de uma resposta. Conta apagada depois da autenticação → 401; senha errada → 403.
async fn check_current_password(
    pool: &PgPool,
    user_id: Uuid,
    password: &str,
) -> Result<(), AppError> {
    let row = sqlx::query!("SELECT password_hash FROM users WHERE id = $1", user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !verify_password(row.password_hash, password.to_string()).await? {
        return Err(AppError::Forbidden(WRONG_PASSWORD));
    }
    Ok(())
}

// Exclusão definitiva, com a senha como confirmação (um token roubado sozinho não apaga a conta).
// Os registros financeiros somem junto (ON DELETE CASCADE no banco).
async fn delete_me(
    State(pool): State<PgPool>,
    current: CurrentUser,
    JsonBody(body): JsonBody,
) -> Result<StatusCode, AppError> {
    let password = confirmation_password(&body, "password")?;
    check_current_password(&pool, current.user.id, password).await?;

    let deleted = sqlx::query!("DELETE FROM users WHERE id = $1", current.user.id)
        .execute(&pool)
        .await?;
    // `rows_affected()`: quantas linhas o DELETE apagou. 0 = apagada por outra requisição no meio do caminho.
    if deleted.rows_affected() == 0 {
        return Err(AppError::Unauthorized);
    }
    Ok(StatusCode::NO_CONTENT)
}

// Resposta da troca de senha: o mesmo formato do login.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NewTokenResponse {
    token: String,
    token_type: &'static str,
    expires_in: u64,
}

// Troca de senha. Derruba todos os tokens da conta (token_version + 1) e devolve um token novo, para o
// dispositivo que trocou continuar logado.
async fn change_password_route(
    State(state): State<AppState>,
    current: CurrentUser,
    JsonBody(body): JsonBody,
) -> Result<Json<NewTokenResponse>, AppError> {
    // Ordem do schema do TS: currentPassword e depois newPassword (regra do cadastro).
    let current_password = confirmation_password(&body, "currentPassword")?;
    let new_password = required_string(&body, "newPassword")?;
    check_new_password(new_password)?;

    check_current_password(&state.pool, current.user.id, current_password).await?;
    let new_hash = hash_password(new_password.to_string()).await?;
    let new_version = change_password(
        &state.pool,
        current.user.id,
        current.token_version,
        &new_hash,
    )
    .await?;

    let token = state
        .tokens
        .create_access_token(current.user.id, new_version)?;
    Ok(Json(NewTokenResponse {
        token,
        token_type: "Bearer",
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
    }))
}

// Hash e versão numa consulta só. O `token_version` do token na condição impede a troca se ele foi revogado no
// meio do caminho (logout ou outra troca de senha em outro dispositivo): 0 linhas → 401, nada muda.
// O incremento é do banco: duas trocas simultâneas não se atropelam. Devolve a versão nova.
pub async fn change_password(
    pool: &PgPool,
    user_id: Uuid,
    token_version: i32,
    new_hash: &str,
) -> Result<i32, AppError> {
    let row = sqlx::query!(
        "UPDATE users SET password_hash = $1, token_version = token_version + 1
         WHERE id = $2 AND token_version = $3
         RETURNING token_version",
        new_hash,
        user_id,
        token_version,
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(row.token_version)
}
