// Estado compartilhado por todas as rotas: o pool do banco e as chaves do JWT.
use std::sync::Arc;

use axum::extract::FromRef;
use sqlx::PgPool;

use crate::{rate_limit::RateLimits, token::TokenKeys};

// `Clone` porque o axum entrega uma cópia do estado a cada requisição. O PgPool já é barato de clonar; as chaves
// ficam num `Arc` (ponteiro com contagem de referências): clonar o Arc só soma 1 na contagem, sem copiar as chaves.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tokens: Arc<TokenKeys>,
    pub limits: Arc<RateLimits>,
}

impl AppState {
    // Rate limit sempre ligado; só os testes desligam (`without_rate_limit`).
    pub fn new(pool: PgPool, jwt_secret: &str) -> Self {
        AppState {
            pool,
            tokens: Arc::new(TokenKeys::new(jwt_secret)),
            limits: Arc::new(RateLimits::new(true)),
        }
    }

    // Para os testes: todos os pedidos vêm do mesmo IP e fazem mais logins por minuto que o limite.
    pub fn without_rate_limit(mut self) -> Self {
        self.limits = Arc::new(RateLimits::new(false));
        self
    }
}

// Deixa um handler pedir só uma parte do estado: `State<PgPool>` funciona num Router<AppState>.
impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}
