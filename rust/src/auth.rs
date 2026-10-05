// Autenticação (o equivalente ao requireAuth + currentUser do src/lib/authenticate.ts).
// No axum não há hook: a rota protegida pede um `CurrentUser` como parâmetro, e este extractor roda antes do
// handler. Se o token não valer, o handler nem roda e a resposta é o 401 padrão.
use axum::{extract::FromRequestParts, http::header::AUTHORIZATION, http::request::Parts};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{error::AppError, state::AppState};

// Colunas do usuário que podem sair numa resposta (sem password_hash nem token_version). O GET /users/me devolve
// exatamente isto.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    #[serde(serialize_with = "crate::dates::serialize_js_iso")]
    pub created_at: OffsetDateTime,
}

// O usuário dono do token e a versão do token (já conferida com o banco). A versão fica fora do PublicUser para
// nunca sair numa resposta; serve para gravações que precisam saber que o token ainda vale (troca de senha).
pub struct CurrentUser {
    pub user: PublicUser,
    pub token_version: i32,
}

// `FromRequestParts` (e não FromRequest): lê só o cabeçalho, sem consumir o corpo. Os extractors de partes rodam
// antes do JsonBody, então quem não está autenticado recebe 401 sem que o corpo seja lido.
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Formato "Bearer <token>". O nome do esquema não diferencia maiúsculas (RFC 7235).
        // Cada passo que falha vira o mesmo 401: `.ok_or(...)` transforma o None em Err, e o `?` sai na hora.
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let mut pieces = header.split(' ');
        let (Some(scheme), Some(token)) = (pieces.next(), pieces.next()) else {
            return Err(AppError::Unauthorized);
        };
        if !scheme.eq_ignore_ascii_case("bearer") || token.is_empty() {
            return Err(AppError::Unauthorized);
        }

        let claims = state
            .tokens
            .verify_access_token(token)
            .ok_or(AppError::Unauthorized)?;

        // Exigir a versão igual na mesma consulta é o que revoga os tokens antigos (logout, troca de senha).
        // Um erro do banco aqui NÃO é 401: o `?` o transforma em 500 (com log), como no TS.
        let user = sqlx::query_as!(
            PublicUser,
            "SELECT id, username, email, role, created_at FROM users WHERE id = $1 AND token_version = $2",
            claims.user_id,
            claims.token_version,
        )
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::Unauthorized)?;

        Ok(CurrentUser {
            user,
            token_version: claims.token_version,
        })
    }
}
