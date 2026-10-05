// Raiz da biblioteca do crate. O main.rs (o binário) e os testes em tests/ usam o que está declarado aqui.
// `pub mod x;` diz ao compilador: existe um módulo `x` (no arquivo src/x.rs ou src/x/mod.rs) e ele é público.
pub mod app;
pub mod auth;
pub mod config;
pub mod dates;
pub mod error;
pub mod json;
pub mod password;
pub mod rate_limit;
pub mod routes;
pub mod state;
pub mod token;
pub mod user_fields;
pub mod validation;
