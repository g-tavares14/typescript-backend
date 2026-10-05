// Monta o Router com todas as rotas (o equivalente ao src/app.ts). Separado do main.rs para os testes montarem
// o mesmo app sem abrir porta.
use axum::Router;
use sqlx::PgPool;

use crate::routes;

pub fn build_app(pool: PgPool) -> Router {
    Router::new()
        .nest("/health", routes::health::router())
        // O PgPool é barato de clonar (por dentro é um ponteiro com contagem de referências para o mesmo pool),
        // então cada requisição recebe a sua cópia sem abrir conexões novas.
        .with_state(pool)
}
