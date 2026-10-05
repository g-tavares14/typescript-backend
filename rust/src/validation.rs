// Peças comuns da validação manual dos corpos (o papel do Zod no TS).
use serde_json::{Map, Value};

use crate::error::AppError;

// Campo ausente ou com tipo JSON errado (inclusive `null`).
pub const REQUIRED: &str = "Campo obrigatório ausente ou inválido";

// Atalho para o erro 400 com uma mensagem fixa.
pub fn bad_request(message: &str) -> AppError {
    AppError::BadRequest(message.to_string())
}

// Lê um campo que precisa ser string. Devolve uma referência (`&str`) para o texto que está dentro do Map:
// nada é copiado. O `'a` (lifetime) diz ao compilador que o &str devolvido vive enquanto o Map viver.
pub fn required_string<'a>(body: &'a Map<String, Value>, field: &str) -> Result<&'a str, AppError> {
    match body.get(field) {
        Some(Value::String(text)) => Ok(text),
        _ => Err(bad_request(REQUIRED)),
    }
}

// Tamanho como o JavaScript conta (`"texto".length`): em unidades UTF-16, não em letras nem em bytes.
// Assim as regras "mínimo 8" e "entre 3 e 50" dão o mesmo resultado nos dois servidores, até com emoji.
pub fn js_length(text: &str) -> usize {
    text.encode_utf16().count()
}
