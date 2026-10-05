// Ponto de entrada do servidor (o equivalente ao src/server.ts).
use std::process;
use std::time::Duration;

use meu_backend::{app::build_app, config::Config, state::AppState};
use sqlx::postgres::PgPoolOptions;

// `#[tokio::main]` é uma macro: transforma a `main` assíncrona numa `main` normal que liga o runtime do tokio
// (o "event loop" que, no Node, já vem pronto) e roda a função dentro dele.
#[tokio::main]
async fn main() {
    // Procura um .env no diretório atual e nos de cima (rodando de rust/, acha o ../.env). Sem .env, segue com
    // o ambiente. O `.ok()` descarta o erro de propósito: o .env é opcional, a validação é do Config.
    dotenvy::dotenv().ok();

    // Liga os logs do `tracing` (o pino do TS). Nível pela variável RUST_LOG (ex.: RUST_LOG=debug); padrão info.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Aqui (e só na inicialização) um erro encerra o processo: fail fast.
    let config = Config::from_env().unwrap_or_else(|message| {
        eprintln!("{message}");
        process::exit(1);
    });

    // acquire_timeout: desiste em 3 s em vez de esperar indefinidamente (como o connectionTimeoutMillis do TS).
    // `connect` já abre uma conexão: se o banco não responder, o servidor não sobe.
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(3))
        .connect(&config.database_url)
        .await
        .unwrap_or_else(|error| {
            eprintln!("Falha ao conectar no banco de dados: {error}");
            process::exit(1);
        });

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port))
        .await
        .unwrap_or_else(|error| {
            eprintln!("Não foi possível abrir a porta {}: {error}", config.port);
            process::exit(1);
        });
    println!("Servidor Rust ouvindo na porta {}", config.port);

    let state = AppState::new(pool, &config.jwt_secret);

    // A cada minuto, os contadores esquecem os IPs que já recuperaram a cota (a memória não cresce sem fim).
    let limits = state.limits.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            limits.retain_recent();
        }
    });

    // `into_make_service_with_connect_info`: cada requisição leva o endereço de quem conectou (o rate limit usa o IP).
    let app = build_app(state).into_make_service_with_connect_info::<std::net::SocketAddr>();
    if let Err(error) = axum::serve(listener, app).await {
        eprintln!("Servidor encerrado com erro: {error}");
        process::exit(1);
    }
}
