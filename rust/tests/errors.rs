// Respostas de erro da API: toda falha sai como { "error": "..." } em português.
// As rotas reais com corpo só chegam na T4; aqui um Router de teste com o mesmo acabamento (`finish`) do app.
use axum::{
    Json, Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use http_body_util::BodyExt; // `.collect()` para ler o corpo inteiro da resposta
use meu_backend::{
    app::finish,
    http::{error::AppError, json::JsonBody},
};
use serde_json::{Value, json};
use tower::ServiceExt;

const INVALID_BODY: &str = "Corpo da requisição inválido: envie um objeto JSON";
const UNSUPPORTED: &str = "Tipo de conteúdo não suportado (use application/json)";

fn test_app() -> Router {
    let router = Router::new()
        // Devolve o objeto recebido: prova que o JsonBody aceitou o corpo.
        .route(
            "/eco",
            post(|JsonBody(body): JsonBody| async move { Json(body) }),
        )
        .route(
            "/falha",
            get(|| async { Err::<(), _>(AppError::internal("senha=segredo no detalhe interno")) }),
        )
        .route("/panico", get(panico));
    finish(router)
}

// O tipo de retorno explícito é necessário: um bloco que só faz panic! tem o tipo `!` ("nunca retorna"), que não
// implementa IntoResponse.
async fn panico() -> &'static str {
    panic!("pânico de teste")
}

async fn send(request: Request<Body>) -> (StatusCode, Value) {
    let response: Response = test_app().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

fn post_eco(content_type: Option<&str>, body: impl Into<Body>) -> Request<Body> {
    let mut builder = Request::post("/eco");
    if let Some(value) = content_type {
        builder = builder.header(header::CONTENT_TYPE, value);
    }
    builder.body(body.into()).unwrap()
}

#[tokio::test]
async fn objeto_json_valido_passa() {
    let (status, body) = send(post_eco(Some("application/json"), r#"{"a":1}"#)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "a": 1 }));
}

#[tokio::test]
async fn corpo_que_nao_e_objeto_json_da_400_invalid_body() {
    for corpo in ["", "{", "null", "[]", "\"texto\"", "123"] {
        let (status, body) = send(post_eco(Some("application/json"), corpo)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "corpo {corpo:?}");
        assert_eq!(body, json!({ "error": INVALID_BODY }), "corpo {corpo:?}");
    }
}

#[tokio::test]
async fn tipo_de_conteudo_que_nao_e_json_da_415() {
    // Diferença para o TS: o Fastify lê text/plain (e corpo sem Content-Type) e responde 400 INVALID_BODY.
    for content_type in [Some("text/plain"), Some("application/xml"), None] {
        let (status, body) = send(post_eco(content_type, "{}")).await;
        assert_eq!(
            status,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "{content_type:?}"
        );
        assert_eq!(body, json!({ "error": UNSUPPORTED }));
    }
}

#[tokio::test]
async fn corpo_acima_de_1_mib_da_413() {
    let big = format!(r#"{{"a":"{}"}}"#, "x".repeat(1024 * 1024));
    let (status, body) = send(post_eco(Some("application/json"), big)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body, json!({ "error": "Corpo da requisição muito grande" }));
}

#[tokio::test]
async fn rota_inexistente_da_404() {
    let (status, body) = send(Request::get("/nao-existe").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, json!({ "error": "Rota não encontrada" }));
}

#[tokio::test]
async fn metodo_nao_permitido_da_405() {
    // Diferença para o TS: o Fastify responde 404 para método errado numa rota que existe.
    let (status, body) = send(Request::delete("/eco").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(body, json!({ "error": "Método não permitido" }));
}

#[tokio::test]
async fn erro_interno_da_500_generico_sem_detalhes() {
    let (status, body) = send(Request::get("/falha").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body, json!({ "error": "Erro interno do servidor" }));
}

#[tokio::test]
async fn panico_no_handler_da_500_generico() {
    let (status, body) = send(Request::get("/panico").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body, json!({ "error": "Erro interno do servidor" }));
}
