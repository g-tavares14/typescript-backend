// Rotas de /users (o equivalente ao src/routes/users.ts). Todas exigem login: cada handler pede um CurrentUser.
use axum::{Json, Router, extract::State, routing::get};
use serde_json::{Map, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::{CurrentUser, PublicUser},
    error::AppError,
    json::JsonBody,
    state::AppState,
    user_fields::{DUPLICATE_USER, parse_email, parse_username},
    validation::{REQUIRED, bad_request},
};

pub fn router() -> Router<AppState> {
    // Duas rotas no mesmo caminho: `get(me).patch(update_me)` junta os métodos.
    Router::new().route("/me", get(me).patch(update_me))
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
