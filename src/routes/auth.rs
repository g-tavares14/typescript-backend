use argon2::{Argon2, PasswordHasher};
use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, Router, routing::post};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, SqlErr};
use serde::Deserialize;

use crate::entities::users;

#[derive(Deserialize)]
struct RegisterRequest {
    username: String,
    password: String,
    email: String,
}

async fn register(
    State(db): State<DatabaseConnection>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, String), (StatusCode, String)> {
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

    // Só chega aqui se todas as validações passaram.
    // Se o hash falhar, o problema é do servidor (500), não de quem mandou os dados (400).
    let senha_hasheada = hash_password(&password).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Erro interno ao processar a senha".to_string(),
        )
    })?;

    // id, role e created_at ficam NotSet: o banco preenche com os DEFAULTs da migration.
    let novo_usuario = users::ActiveModel {
        username: Set(username),
        email: Set(email),
        password_hash: Set(senha_hasheada),
        ..Default::default()
    };

    match novo_usuario.insert(&db).await {
        // O insert devolve o Model completo, já com o id gerado pelo banco.
        Ok(usuario) => Ok((
            StatusCode::CREATED,
            format!("Usuário {} criado com id {}", usuario.username, usuario.id),
        )),
        Err(erro) if matches!(erro.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) => {
            Err((
                StatusCode::CONFLICT,
                "Email ou username já cadastrado".to_string(),
            ))
        }
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Erro interno ao criar o usuário".to_string(),
        )),
    }
}

// No argon2 0.6, o hash_password já gera um salt aleatório por dentro.
fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let hash = Argon2::default().hash_password(password.as_bytes())?;
    Ok(hash.to_string())
}

pub fn auth_routes() -> Router<DatabaseConnection> {
    Router::new().route("/register", post(register))
}
