use axum::http::StatusCode;
use axum::{Json, Router, routing::post};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
    email: String,
}

async fn register(Json(payload): Json<RegisterRequest>) -> Result<String, (StatusCode, String)> {
    // Normalização: tira espaços das pontas e deixa o email em minúsculas.
    // A senha NÃO é alterada: espaços e maiúsculas fazem parte dela.
    let username = payload.username.trim().to_string();
    let email = payload.email.trim().to_lowercase();
    let password = payload.password;

    // .chars().count() conta letras; .len() contaria bytes ("joão" tem 4 letras e 5 bytes).
    let username_len = username.chars().count();
    if username_len < 3 || username_len > 50 {
        return Err((
            StatusCode::BAD_REQUEST,
            "O username deve ter entre 3 e 50 caracteres".to_string(),
        ));
    }

    // split_once devolve Some((antes, depois)) se houver um '@', ou None se não houver.
    let email_valido = match email.split_once('@') {
        Some((antes, depois)) => !antes.is_empty() && !depois.is_empty(),
        None => false,
    };
    if !email_valido {
        return Err((StatusCode::BAD_REQUEST, "Email inválido".to_string()));
    }

    if password.chars().count() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            "A senha deve ter no mínimo 8 caracteres".to_string(),
        ));
    }

    Ok(format!("Usuário {username} ({email}) passou na validação"))
}

pub fn auth_routes() -> Router<PgPool> {
    Router::new().route("/register", post(register))
}
