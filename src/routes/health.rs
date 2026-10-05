use axum::{Router, extract::State, http::StatusCode, routing::get};
use sqlx::PgPool;

use crate::state::AppState;

// O Router deste grupo. `Router<AppState>` = um Router que ainda precisa receber o estado
// (quem entrega é o build_app, com `.with_state(state)`). O handler pede só o PgPool (ver FromRef em state.rs).
pub fn router() -> Router<AppState> {
    Router::new().route("/", get(health))
}

// 200 se o banco responde, 503 se não. Sem corpo, como no TS.
// `State(pool)` é um extractor: o axum tira o estado do Router e entrega para o handler já desembrulhado.
async fn health(State(pool): State<PgPool>) -> StatusCode {
    // `&pool`: emprestamos o pool para a consulta (sem copiar nem transferir a posse).
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
    }
}
