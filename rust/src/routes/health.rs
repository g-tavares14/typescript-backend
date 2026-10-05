use axum::{Router, extract::State, http::StatusCode, routing::get};
use sqlx::PgPool;

// O Router deste grupo. `Router<PgPool>` = um Router que ainda precisa receber um PgPool como estado
// (quem entrega é o build_app, com `.with_state(pool)`).
pub fn router() -> Router<PgPool> {
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
