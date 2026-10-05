// Um registro financeiro no formato da API.
use serde::Serialize;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

// Um registro como a API mostra (nunca o user_id). Os nomes do banco viram os da API.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicTransaction {
    pub id: Uuid,
    #[serde(rename = "type")]
    // `type` é palavra reservada em Rust: o campo se chama kind e sai como "type"
    pub kind: String,
    pub amount: i64,
    pub description: String,
    #[serde(serialize_with = "crate::validation::dates::serialize_date")]
    pub date: Date,
    #[serde(serialize_with = "crate::validation::dates::serialize_js_iso")]
    pub created_at: OffsetDateTime,
    #[serde(serialize_with = "crate::validation::dates::serialize_js_iso")]
    pub updated_at: OffsetDateTime,
}
