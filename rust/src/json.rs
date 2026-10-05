// Extractor do corpo JSON. Usa o Json do axum por dentro, mas:
// - só aceita um OBJETO JSON (Map): `null`, array, texto e número viram INVALID_BODY;
// - troca as rejeições do axum (em inglês, com status próprios) por AppError.
// A validação dos campos fica nas rotas, lendo o Map campo a campo.
use axum::{
    Json,
    extract::{FromRequest, Request, rejection::JsonRejection},
};
use serde_json::{Map, Value};

use crate::error::{AppError, INVALID_BODY};

// Struct "tupla" com um campo só (newtype): dá um nome e um comportamento próprio a um Map.
pub struct JsonBody(pub Map<String, Value>);

// Implementar FromRequest é o que deixa usar `JsonBody` como parâmetro de um handler.
// `S` é o tipo do estado do Router; este extractor não usa o estado, então aceita qualquer um.
impl<S: Send + Sync> FromRequest<S> for JsonBody {
    type Rejection = AppError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<Map<String, Value>>::from_request(request, state).await {
            Ok(Json(map)) => Ok(JsonBody(map)),
            Err(rejection) => Err(rejection_to_error(rejection)),
        }
    }
}

fn rejection_to_error(rejection: JsonRejection) -> AppError {
    match rejection {
        // Sem Content-Type JSON (inclusive text/plain e corpo sem Content-Type): 415. Diferença para o TS
        // (o Fastify tem parser de texto e responde 400); ver "Diferenças para o front" na spec.
        JsonRejection::MissingJsonContentType(_) => AppError::UnsupportedMediaType,
        // JSON malformado ou vazio (sintaxe) e JSON válido que não é objeto (dados): 400 INVALID_BODY.
        // O axum responderia 400 e 422; aqui os dois são o mesmo erro para o cliente.
        JsonRejection::JsonSyntaxError(_) | JsonRejection::JsonDataError(_) => {
            AppError::BadRequest(INVALID_BODY.to_string())
        }
        // Falha ao ler o corpo: acima do limite (413) ou outra (conexão cortada, Content-Length errado).
        JsonRejection::BytesRejection(error) if error.status().as_u16() == 413 => {
            AppError::PayloadTooLarge
        }
        JsonRejection::BytesRejection(_) => AppError::BadRequest(INVALID_BODY.to_string()),
        // JsonRejection é `#[non_exhaustive]`: uma versão nova do axum pode criar variantes. Elas caem aqui,
        // com um 400 genérico, e o tipo vai para o log (sem a mensagem, que pode citar o corpo).
        other => {
            tracing::warn!(status = %other.status(), "Rejeição de JSON sem mapeamento");
            AppError::BadRequest("Requisição inválida".to_string())
        }
    }
}
