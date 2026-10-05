// Erro da aplicação: toda resposta de erro da API sai daqui, como { "error": "..." } (o equivalente ao sendError
// do src/lib/errors.ts). Cada variante do enum é um tipo de falha; o `IntoResponse` abaixo diz o status e a
// mensagem de cada uma.
use std::error::Error;

use axum::{
    Json,
    http::{
        StatusCode,
        header::{RETRY_AFTER, WWW_AUTHENTICATE},
    },
    response::{IntoResponse, Response},
};
use serde_json::json;

// Mensagem para corpo que não é um objeto JSON utilizável (ausente, vazio, malformado, `null`, array, texto...).
pub const INVALID_BODY: &str = "Corpo da requisição inválido: envie um objeto JSON";

// Qualquer erro, de qualquer tipo, "numa caixa": Box<dyn Error> guarda o valor na heap e só promete que ele
// implementa a trait Error. Send + Sync deixam o erro atravessar threads (o tokio pode trocar a tarefa de thread).
pub type BoxError = Box<dyn Error + Send + Sync>;

// `#[derive(Debug, thiserror::Error)]` gera a implementação da trait Error; o `#[error("...")]` de cada variante
// vira o texto do Display. Só as mensagens 4xx chegam ao cliente; o Internal tem Display só para o log.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // 400 com a mensagem da validação (ex.: "Email inválido").
    #[error("{0}")]
    BadRequest(String),
    // 409 com uma mensagem fixa (ex.: email ou username já cadastrado).
    #[error("{0}")]
    Conflict(&'static str),
    // 401 padrão de toda falha de autenticação (sem token, token inválido ou revogado, conta inexistente).
    #[error("Não autenticado")]
    Unauthorized,
    // 401 do login (email ou senha errados): a mesma mensagem para os dois casos.
    #[error("Email ou senha inválidos")]
    InvalidCredentials,
    // 403 com uma mensagem fixa (ex.: senha de confirmação errada; o token é válido, então não é 401).
    #[error("{0}")]
    Forbidden(&'static str),
    // 404 de um recurso (ex.: "Registro não encontrado"), diferente do 404 de rota inexistente.
    #[error("{0}")]
    NotFoundMessage(&'static str),
    // 429 do rate limit, com o tempo de espera para o header Retry-After.
    #[error("Muitas tentativas. Tente novamente mais tarde.")]
    TooManyRequests { retry_after_seconds: u64 },
    #[error("Rota não encontrada")]
    NotFound,
    #[error("Método não permitido")]
    MethodNotAllowed,
    #[error("Corpo da requisição muito grande")]
    PayloadTooLarge,
    #[error("Tipo de conteúdo não suportado (use application/json)")]
    UnsupportedMediaType,
    // 500: o detalhe vai só para o log; o cliente recebe a mensagem genérica.
    #[error("erro interno: {0}")]
    Internal(BoxError),
}

impl AppError {
    // Atalho para criar um Internal a partir de qualquer erro (ou texto). `impl Into<BoxError>` aceita qualquer
    // tipo que saiba se converter numa caixa de erro: sqlx::Error, String, &str...
    pub fn internal(error: impl Into<BoxError>) -> Self {
        AppError::Internal(error.into())
    }

    fn status(&self) -> StatusCode {
        // `match` em enum: o compilador obriga a tratar todas as variantes. Uma variante nova sem status aqui
        // não compila.
        match self {
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Unauthorized | AppError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::NotFound | AppError::NotFoundMessage(_) => StatusCode::NOT_FOUND,
            AppError::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            AppError::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            AppError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            AppError::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// `?` num handler com um sqlx::Error vira AppError::Internal sozinho: o `?` chama `From::from` no erro.
impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        AppError::internal(error)
    }
}

// Como o axum transforma um AppError numa resposta HTTP. Todo handler que devolve Result<_, AppError> usa isto.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let message = match &self {
            AppError::Internal(error) => {
                // O sqlx::Error não carrega os valores ligados à consulta (email, hash...): pode ir para o log.
                tracing::error!(error = %error, "Erro interno");
                "Erro interno do servidor".to_string()
            }
            other => other.to_string(),
        };
        let body = Json(json!({ "error": message }));
        // O 401 de autenticação diz ao cliente qual esquema usar (RFC 7235). O do login não: lá não há token.
        if let AppError::TooManyRequests {
            retry_after_seconds,
        } = self
        {
            return (
                self.status(),
                [(RETRY_AFTER, retry_after_seconds.to_string())],
                body,
            )
                .into_response();
        }
        if matches!(self, AppError::Unauthorized) {
            return (self.status(), [(WWW_AUTHENTICATE, "Bearer")], body).into_response();
        }
        (self.status(), body).into_response()
    }
}
