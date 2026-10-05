// O usuário no formato da API.
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

// Colunas do usuário que podem sair numa resposta (sem password_hash nem token_version). O GET /users/me devolve
// exatamente isto.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    #[serde(serialize_with = "crate::validation::dates::serialize_js_iso")]
    pub created_at: OffsetDateTime,
}
