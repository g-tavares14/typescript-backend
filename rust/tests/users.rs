// /users/me em casos que a paridade HTTP não alcança (equivalentes aos testes só-TS de users-update-delete.test.ts).
use meu_backend::{
    error::AppError,
    routes::users::{ProfileChanges, update_profile},
};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

async fn test_pool() -> sqlx::PgPool {
    dotenvy::from_filename_override("../.env.test").expect(".env.test não encontrado");
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL não definida no .env.test");
    PgPoolOptions::new().connect(&url).await.unwrap()
}

#[tokio::test]
async fn patch_de_conta_apagada_no_meio_do_caminho_da_401() {
    // Uma conta que não existe é o que o UPDATE encontra quando ela foi apagada depois da autenticação.
    let changes = ProfileChanges {
        username: Some("novo_nome".to_string()),
        email: None,
    };

    let result = update_profile(&test_pool().await, Uuid::new_v4(), changes).await;

    assert!(matches!(result, Err(AppError::Unauthorized)));
}
