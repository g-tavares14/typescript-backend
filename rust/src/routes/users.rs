// Rotas de /users (o equivalente ao src/routes/users.ts). Todas exigem login: cada handler pede um CurrentUser.
use axum::{Json, Router, routing::get};

use crate::{
    auth::{CurrentUser, PublicUser},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route("/me", get(me))
}

// O extractor já validou o token e leu o usuário: aqui é só devolver.
async fn me(current: CurrentUser) -> Json<PublicUser> {
    Json(current.user)
}
