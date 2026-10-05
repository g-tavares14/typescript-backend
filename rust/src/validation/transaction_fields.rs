// Regras dos campos de um registro financeiro (as mesmas no POST e no PATCH /transactions).
use serde_json::Value;
use time::Date;

use crate::http::error::AppError;
use crate::validation::dates::parse_iso_date;
use crate::validation::{REQUIRED, bad_request, js_length};

const TYPE_ERROR: &str = "O tipo deve ser income ou expense";
const AMOUNT_ERROR: &str = "O valor deve ser um número inteiro de centavos maior que zero";
const DESCRIPTION_ERROR: &str = "A descrição deve ter entre 1 e 200 caracteres";
const DATE_ERROR: &str = "Data inválida (use AAAA-MM-DD)";
const MAX_AMOUNT_CENTS: i64 = 100_000_000_000; // R$ 1 bilhão

// Convenção do projeto: tipo JSON errado (inclusive null) → REQUIRED; tipo certo com valor fora da regra →
// a mensagem do campo. Cada função recebe o valor do campo já tirado do corpo.

pub fn parse_type(value: &Value) -> Result<String, AppError> {
    match value {
        // `as_str()` empresta o texto da String para comparar com os literais.
        Value::String(text) => match text.as_str() {
            "income" | "expense" => Ok(text.clone()),
            _ => Err(bad_request(TYPE_ERROR)),
        },
        _ => Err(bad_request(REQUIRED)),
    }
}

// Centavos inteiros, de 1 a R$ 1 bilhão. "1990" (string) é tipo errado; 19.9 é valor inválido.
// O JSON não distingue 1000 de 1000.0 (no JavaScript são o mesmo número), então 1000.0 é aceito como 1000.
pub fn parse_amount(value: &Value) -> Result<i64, AppError> {
    let Value::Number(number) = value else {
        return Err(bad_request(REQUIRED));
    };
    let cents = match number.as_i64() {
        Some(integer) => integer,
        None => {
            let float = number.as_f64().unwrap_or(f64::NAN);
            // `fract()` é a parte depois da vírgula: 19.9 → 0.9. Só aceita número inteiro dentro do teto.
            if float.fract() != 0.0 || float.abs() > MAX_AMOUNT_CENTS as f64 {
                return Err(bad_request(AMOUNT_ERROR));
            }
            float as i64
        }
    };
    if !(1..=MAX_AMOUNT_CENTS).contains(&cents) {
        return Err(bad_request(AMOUNT_ERROR));
    }
    Ok(cents)
}

pub fn parse_description(value: &Value) -> Result<String, AppError> {
    let Value::String(text) = value else {
        return Err(bad_request(REQUIRED));
    };
    let description = text.trim();
    if !(1..=200).contains(&js_length(description)) {
        return Err(bad_request(DESCRIPTION_ERROR));
    }
    Ok(description.to_string())
}

pub fn parse_date(value: &Value) -> Result<Date, AppError> {
    let Value::String(text) = value else {
        return Err(bad_request(REQUIRED));
    };
    parse_date_text(text)
}

// A regra da data sobre o texto: serve ao corpo (parse_date) e aos filtros from/to da query.
pub fn parse_date_text(text: &str) -> Result<Date, AppError> {
    parse_iso_date(text).ok_or_else(|| bad_request(DATE_ERROR))
}
