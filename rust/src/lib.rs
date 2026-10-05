// Raiz da biblioteca do crate. O main.rs (o binário) e os testes em tests/ usam o que está declarado aqui.
// `pub mod x;` diz ao compilador: existe um módulo `x` (no arquivo src/x.rs ou src/x/mod.rs) e ele é público.
pub mod app;
pub mod config;
pub mod error;
pub mod json;
pub mod routes;
