// Respostas de erro da API: toda falha sai como { "error": "..." } em português.
// As rotas reais com corpo só chegam na T4; aqui um Router de teste com o mesmo acabamento (`finish`) do app.
mod common;

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

// ---- No app de verdade (equivalente aos testes de app do test/errors.test.ts) ----

const REQUIRED: &str = "Campo obrigatório ausente ou inválido";

// Corpos que não são um objeto JSON utilizável, com a resposta do Rust para cada um. Os dois 415 e o __proto__ são
// diferenças para o TS (SPEC-migracao-rust.md, "Diferenças para o front"): o axum recusa o tipo de conteúdo, e
// `__proto__` é uma chave comum (o corpo segue para a validação dos campos).
fn invalid_bodies() -> Vec<(
    &'static str,
    Option<&'static str>,
    &'static str,
    StatusCode,
    &'static str,
)> {
    let json_type = Some("application/json");
    vec![
        (
            "sem corpo e sem content-type",
            None,
            "",
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            UNSUPPORTED,
        ),
        (
            "application/json vazio",
            json_type,
            "",
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "JSON malformado",
            json_type,
            "{ruim",
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "JSON com __proto__",
            json_type,
            r#"{"__proto__":{"admin":true}}"#,
            StatusCode::BAD_REQUEST,
            REQUIRED,
        ),
        (
            "null",
            json_type,
            "null",
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "array",
            json_type,
            "[]",
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "string JSON",
            json_type,
            r#""x""#,
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "text/plain",
            Some("text/plain"),
            "oi",
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            UNSUPPORTED,
        ),
    ]
}

#[tokio::test]
async fn corpo_invalido_tem_a_mesma_resposta_nas_3_rotas_com_corpo() {
    let app = common::TestApp::new().await;
    let token = app.register_and_login().await;

    for url in ["/auth/register", "/auth/login", "/transactions"] {
        for (case, content_type, body, status, message) in invalid_bodies() {
            let mut request = app.post(url).raw_body(body);
            if let Some(content_type) = content_type {
                request = request.header("content-type", content_type);
            }
            // Só a rota protegida precisa de token.
            if url == "/transactions" {
                request = request.bearer(&token);
            }

            let response = request.send().await;

            assert_eq!(response.status, status, "{url}: {case}");
            assert_eq!(
                response.json(),
                json!({ "error": message }),
                "{url}: {case}"
            );
        }
    }
}

#[tokio::test]
async fn com_um_objeto_json_as_mensagens_continuam_as_de_cada_campo() {
    let app = common::TestApp::new().await;

    for url in ["/auth/register", "/auth/login"] {
        let response = app.post(url).json(&json!({})).send().await;
        assert_eq!(response.json(), json!({ "error": REQUIRED }), "{url}");
    }
}

#[tokio::test]
async fn no_app_xml_da_415_e_corpo_acima_de_1_mib_da_413() {
    let app = common::TestApp::new().await;

    let xml = app
        .post("/auth/login")
        .header("content-type", "application/xml")
        .raw_body("<a/>")
        .send()
        .await;
    let big = app
        .post("/auth/login")
        .json(&json!({ "email": "a".repeat(1024 * 1024) }))
        .send()
        .await;

    assert_eq!(xml.status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(xml.json(), json!({ "error": UNSUPPORTED }));
    assert_eq!(big.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        big.json(),
        json!({ "error": "Corpo da requisição muito grande" })
    );
}

#[tokio::test]
async fn rota_inexistente_da_404_sem_repetir_a_url() {
    let app = common::TestApp::new().await;

    let response = app.get("/nada?segredo=1").send().await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.json(), json!({ "error": "Rota não encontrada" }));
    assert!(!response.body.contains("nada"));
}

#[tokio::test]
async fn metodo_inexistente_numa_rota_que_existe_da_405() {
    // Diferença para o TS (o Fastify responde 404).
    let app = common::TestApp::new().await;

    let response = app.delete("/transactions").send().await;

    assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.json(), json!({ "error": "Método não permitido" }));
}

#[tokio::test]
async fn url_malformada_da_404_sem_repetir_a_url() {
    // Diferença para o TS (o Fastify responde 400 "URL inválida"): o axum não decodifica o caminho para escolher a
    // rota, então não acha nenhuma.
    let app = common::TestApp::new().await;

    let response = app.get("/%E0%A4%A").send().await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.json(), json!({ "error": "Rota não encontrada" }));
    assert!(!response.body.contains("E0"));
}

#[tokio::test]
async fn o_401_vem_antes_de_qualquer_erro_de_corpo_nas_rotas_protegidas() {
    let app = common::TestApp::new().await;
    let big = json!({ "description": "a".repeat(1024 * 1024) }).to_string();
    let cases: [(Option<&str>, &str); 4] = [
        (Some("application/json"), "{ruim"),
        (Some("application/xml"), "<a/>"),
        (Some("application/json"), &big),
        (None, ""),
    ];

    for (content_type, body) in cases {
        let mut request = app.post("/transactions").raw_body(body.to_string());
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        common::assert_unauthorized(&request.send().await);
    }
}

#[tokio::test]
async fn o_429_do_rate_limit_tem_a_mensagem_em_portugues() {
    let app = common::TestApp::with_rate_limit().await;
    for _ in 0..5 {
        app.post("/auth/login").json(&json!({})).send().await;
    }

    let blocked = app.post("/auth/login").json(&json!({})).send().await;

    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        blocked.json(),
        json!({ "error": "Muitas tentativas. Tente novamente mais tarde." })
    );
}

// Precisa do servidor de verdade: quem usa o Content-Length para cortar o corpo é o hyper (o servidor HTTP por baixo
// do axum), e o `oneshot` entrega o corpo direto ao Router, sem passar por ele. O teste abre uma porta qualquer,
// escreve a requisição crua no socket e lê a resposta.
#[tokio::test]
async fn content_length_que_nao_confere_com_o_corpo_e_corpo_invalido() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let app = common::TestApp::new().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap(); // porta 0: o sistema escolhe
    let address = listener.local_addr().unwrap();
    let router = meu_backend::app::build_app(app.state.clone());
    tokio::spawn(async move {
        let service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
        axum::serve(listener, service).await.unwrap();
    });

    // Content-Length 5: o servidor lê só `{"ema` como corpo.
    let body = r#"{"email":"a@b.com","password":"x"}"#;
    let raw = format!(
        "POST /auth/login HTTP/1.1\r\nHost: teste\r\nContent-Type: application/json\r\n\
         Content-Length: 5\r\nConnection: close\r\n\r\n{body}"
    );
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(raw.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();

    assert!(response.starts_with("HTTP/1.1 400"), "{response}");
    assert!(
        response.ends_with(&json!({ "error": INVALID_BODY }).to_string()),
        "{response}"
    );
}
