// Raiz da biblioteca do crate. O main.rs (o binário) e os testes em tests/ usam o que está declarado aqui.
// `pub mod x;` diz ao compilador: existe um módulo `x`, no arquivo src/x.rs; os submódulos dele ficam em src/x/.
pub mod app;
pub mod config;
pub mod http;
pub mod models;
pub mod routes;
pub mod security;
pub mod state;
pub mod validation;
