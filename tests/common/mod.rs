// Helpers dos testes de integração (o equivalente ao test/helpers.ts + test/global-setup.ts do vitest).
// Cada arquivo em tests/ é um crate separado e declara `mod common;` para usar este módulo. Um arquivo que não usa
// algum helper daria aviso de código morto, por isso o `allow` abaixo vale para o módulo inteiro.
#![allow(dead_code)]

use std::net::SocketAddr;

use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use meu_backend::{app::build_app, state::AppState};
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, Connection, PgConnection, PgPool};
use tokio::sync::{Mutex, MutexGuard, OnceCell};
use tower::ServiceExt;

// Lê o .env.test (o mesmo banco que o vitest usava) e só aceita banco terminado em _test: os testes apagam tudo.
pub fn test_env() -> (String, String) {
    dotenvy::from_filename_override(".env.test").expect(".env.test não encontrado");
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL não definida no .env.test");
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET não definido no .env.test");
    let name = url.rsplit('/').next().unwrap_or_default();
    assert!(
        name.ends_with("_test"),
        "Os testes só podem rodar num banco terminado em \"_test\" (recebido: {name})"
    );
    (url, secret)
}

// Uma vez por arquivo de teste: cria o banco de testes, se preciso, e aplica as migrations.
// `OnceCell` do tokio: a primeira chamada roda a inicialização; as outras esperam ela terminar e seguem.
static DATABASE_READY: OnceCell<()> = OnceCell::const_new();

async fn prepare_database(url: &str) {
    let (admin_url, name) = url
        .rsplit_once('/')
        .expect("DATABASE_URL sem nome de banco");
    let mut admin = PgConnection::connect(&format!("{admin_url}/postgres"))
        .await
        .expect("Postgres fora do ar? Rode `docker compose up -d`");

    // Banco criado por outra ferramenta (o drizzle, na época do TS) não tem a tabela de controle do sqlx: o
    // `migrate` tentaria criar as tabelas de novo. Como o banco de testes é descartável, ele é recriado.
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(name)
            .fetch_one(&mut admin)
            .await
            .unwrap();
    // O nome não pode ser parâmetro ($1) num DROP/CREATE DATABASE: vai no texto do SQL. O sqlx 0.9 só aceita SQL
    // montado em tempo de execução dentro de `AssertSqlSafe` ("eu conferi"); a conferência é esta.
    assert!(
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
        "nome de banco inesperado: {name}"
    );
    let recreate = exists && !managed_by_sqlx(url).await;
    if recreate {
        sqlx::raw_sql(AssertSqlSafe(format!(
            r#"DROP DATABASE "{name}" WITH (FORCE)"#
        )))
        .execute(&mut admin)
        .await
        .unwrap();
    }
    if !exists || recreate {
        sqlx::raw_sql(AssertSqlSafe(format!(r#"CREATE DATABASE "{name}""#)))
            .execute(&mut admin)
            .await
            .unwrap();
    }

    let mut connection = PgConnection::connect(url).await.unwrap();
    // `migrate!()` embute os arquivos de migrations/ no binário de teste, na compilação.
    sqlx::migrate!()
        .run(&mut connection)
        .await
        .expect("falha ao aplicar as migrations no banco de testes");
}

// Conecta e procura a tabela do sqlx. Banco que não existe também conta como "não gerenciado".
async fn managed_by_sqlx(url: &str) -> bool {
    let Ok(mut connection) = PgConnection::connect(url).await else {
        return false;
    };
    sqlx::query_scalar("SELECT to_regclass('public._sqlx_migrations') IS NOT NULL")
        .fetch_one(&mut connection)
        .await
        .unwrap_or(false)
}

// Pool do banco de testes, já migrado. Cada teste cria o seu: o pool fica preso ao runtime do tokio que o criou,
// e cada #[tokio::test] tem um runtime próprio.
pub async fn test_pool() -> PgPool {
    let (url, _) = test_env();
    DATABASE_READY.get_or_init(|| prepare_database(&url)).await;
    PgPool::connect(&url).await.unwrap()
}

// Os testes de um arquivo rodam em paralelo (threads), mas dividem o banco: quem apaga as tabelas não pode correr
// junto com quem está usando. Cada TestApp segura esta trava até o fim do teste (o `fileParallelism: false` do
// vitest, aqui por teste). Arquivos diferentes já rodam um depois do outro.
static DATABASE_LOCK: Mutex<()> = Mutex::const_new(());

pub const DEFAULT_USERNAME: &str = "joao";
pub const DEFAULT_EMAIL: &str = "joao@email.com";
pub const DEFAULT_PASSWORD: &str = "senha123";

pub fn default_user() -> Value {
    json!({ "username": DEFAULT_USERNAME, "email": DEFAULT_EMAIL, "password": DEFAULT_PASSWORD })
}

// O app de verdade (mesmas rotas e respostas de erro), ligado ao banco de testes, que começa vazio.
pub struct TestApp {
    pub pool: PgPool,
    pub state: AppState,
    router: Router,
    // O `_` no nome: o campo só existe para a trava durar o mesmo tempo que o TestApp.
    _lock: MutexGuard<'static, ()>,
}

impl TestApp {
    // Rate limit DESLIGADO: todas as requisições "vêm" do mesmo IP e os testes fazem muitos logins.
    pub async fn new() -> Self {
        Self::build(false).await
    }

    // Com os limites reais (só os testes de rate limit).
    pub async fn with_rate_limit() -> Self {
        Self::build(true).await
    }

    async fn build(rate_limit: bool) -> Self {
        let lock = DATABASE_LOCK.lock().await;
        let pool = test_pool().await;
        sqlx::query("TRUNCATE TABLE users, transactions")
            .execute(&pool)
            .await
            .unwrap();
        let (_, secret) = test_env();
        let mut state = AppState::new(pool.clone(), &secret);
        if !rate_limit {
            state = state.without_rate_limit();
        }
        let router = build_app(state.clone());
        Self {
            pool,
            state,
            router,
            _lock: lock,
        }
    }

    pub fn request(&self, method: Method, uri: &str) -> TestRequest<'_> {
        TestRequest {
            app: self,
            method,
            uri: uri.to_string(),
            headers: Vec::new(),
            body: None,
            ip: "127.0.0.1".to_string(),
        }
    }

    pub fn get(&self, uri: &str) -> TestRequest<'_> {
        self.request(Method::GET, uri)
    }
    pub fn post(&self, uri: &str) -> TestRequest<'_> {
        self.request(Method::POST, uri)
    }
    pub fn patch(&self, uri: &str) -> TestRequest<'_> {
        self.request(Method::PATCH, uri)
    }
    pub fn put(&self, uri: &str) -> TestRequest<'_> {
        self.request(Method::PUT, uri)
    }
    pub fn delete(&self, uri: &str) -> TestRequest<'_> {
        self.request(Method::DELETE, uri)
    }

    // Atalhos para o "Arrange": cadastram e fazem login pelas rotas de verdade (como o registerUser/loginUser).
    pub async fn register(&self, user: Value) -> Value {
        let response = self.post("/auth/register").json(&user).send().await;
        assert_eq!(
            response.status,
            StatusCode::CREATED,
            "Cadastro falhou no arrange do teste: {}",
            response.body
        );
        response.json()
    }

    pub async fn login(&self, email: &str, password: &str) -> String {
        let response = self
            .post("/auth/login")
            .json(&json!({ "email": email, "password": password }))
            .send()
            .await;
        assert_eq!(
            response.status,
            StatusCode::OK,
            "Login falhou no arrange do teste: {}",
            response.body
        );
        response.json()["token"].as_str().unwrap().to_string()
    }

    // Cadastra o usuário padrão e devolve o token dele: o arrange mais comum.
    pub async fn register_and_login(&self) -> String {
        self.register_and_login_as(default_user()).await
    }

    // O mesmo para qualquer usuário ({ username, email, password }).
    pub async fn register_and_login_as(&self, user: Value) -> String {
        self.register(user.clone()).await;
        let email = user["email"].as_str().unwrap();
        self.login(email, user["password"].as_str().unwrap()).await
    }
}

// Uma requisição em construção (builder pattern): cada método consome o builder e devolve um novo, e o `send`
// termina a cadeia. Ex.: app.post("/auth/login").json(&corpo).send().await
pub struct TestRequest<'a> {
    app: &'a TestApp,
    method: Method,
    uri: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    ip: String,
}

impl TestRequest<'_> {
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn bearer(self, token: &str) -> Self {
        self.header("authorization", &format!("Bearer {token}"))
    }

    // Objeto serializado + Content-Type JSON (como o payload objeto do inject).
    pub fn json(self, body: &Value) -> Self {
        self.header("content-type", "application/json")
            .raw_body(body.to_string())
    }

    // Corpo cru, sem Content-Type (para os testes de corpo malformado).
    pub fn raw_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    // De onde a requisição "vem": o ConnectInfo é o que o servidor real coloca em cada uma (o rate limit usa o IP).
    pub fn ip(mut self, ip: &str) -> Self {
        self.ip = ip.to_string();
        self
    }

    pub async fn send(self) -> TestResponse {
        let address: SocketAddr = format!("{}:5000", self.ip).parse().unwrap();
        let mut builder = Request::builder()
            .method(self.method)
            .uri(self.uri)
            .extension(ConnectInfo(address));
        for (name, value) in &self.headers {
            builder = builder.header(name, value);
        }
        let request = builder
            .body(self.body.map(Body::from).unwrap_or_else(Body::empty))
            .unwrap();

        // `clone()`: o oneshot consome o Router; os clones dividem o mesmo estado (e os mesmos contadores).
        let response = self.app.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        TestResponse {
            status,
            headers,
            body: String::from_utf8(bytes.to_vec()).unwrap(),
        }
    }
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

impl TestResponse {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|_| panic!("resposta não é JSON: {:?}", self.body))
    }

    pub fn header(&self, name: header::HeaderName) -> Option<&str> {
        self.headers
            .get(name)
            .and_then(|value: &HeaderValue| value.to_str().ok())
    }
}

// Toda falha de autenticação tem a mesma resposta: 401, WWW-Authenticate e a mesma mensagem.
pub fn assert_unauthorized(response: &TestResponse) {
    assert_eq!(
        response.status,
        StatusCode::UNAUTHORIZED,
        "{}",
        response.body
    );
    assert_eq!(response.header(header::WWW_AUTHENTICATE), Some("Bearer"));
    assert_eq!(response.json(), json!({ "error": "Não autenticado" }));
}

// Um erro 400 com a mensagem dada.
pub fn assert_bad_request(response: &TestResponse, message: &str) {
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.body
    );
    assert_eq!(response.json(), json!({ "error": message }));
}
