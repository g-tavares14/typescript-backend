// Monta o Router com todas as rotas (o equivalente ao src/app.ts). Separado do main.rs para os testes montarem
// o mesmo app sem abrir porta.
use axum::{Router, extract::DefaultBodyLimit, response::IntoResponse};
use tower_http::catch_panic::CatchPanicLayer;

use crate::{error::AppError, routes, state::AppState};

// Limite do corpo: 1 MiB, igual ao padrão do Fastify (o padrão do axum é 2 MB).
const BODY_LIMIT_BYTES: usize = 1024 * 1024;

pub fn build_app(state: AppState) -> Router {
    let router = Router::new()
        .nest("/health", routes::health::router())
        .nest("/auth", routes::auth::router());
    // O estado é clonado para cada requisição (barato: ver src/state.rs).
    finish(router).with_state(state)
}

// Acabamento comum a qualquer Router da API: respostas de 404/405, limite do corpo e pânico → 500.
// Genérico em `S` (o tipo do estado) para os testes aplicarem o mesmo acabamento num Router de teste.
pub fn finish<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    router
        .fallback(|| async { AppError::NotFound })
        // Rota existe, mas não com este método. Diferença para o TS: o Fastify responde 404.
        .method_not_allowed_fallback(|| async { AppError::MethodNotAllowed })
        .layer(DefaultBodyLimit::max(BODY_LIMIT_BYTES))
        // Um panic num handler derrubaria só a conexão; com esta camada vira o mesmo 500 genérico (com log).
        .layer(CatchPanicLayer::custom(|_panic| {
            AppError::internal("panic num handler").into_response()
        }))
}
