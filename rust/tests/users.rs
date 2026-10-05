// /users/me em casos que a paridade HTTP não alcança (equivalentes aos testes só-TS de users-update-delete.test.ts).
mod common;

use common::TestApp;
use meu_backend::{
    http::error::AppError,
    routes::users::{ProfileChanges, change_password, check_current_password, update_profile},
};
use uuid::Uuid;

#[tokio::test]
async fn patch_de_conta_apagada_no_meio_do_caminho_da_401() {
    // Uma conta que não existe é o que o UPDATE encontra quando ela foi apagada depois da autenticação.
    let changes = ProfileChanges {
        username: Some("novo_nome".to_string()),
        email: None,
    };

    let result = update_profile(&TestApp::new().await.pool, Uuid::new_v4(), changes).await;

    assert!(matches!(result, Err(AppError::Unauthorized)));
}

#[tokio::test]
async fn troca_de_senha_com_token_revogado_no_meio_do_caminho_da_401_e_nao_muda_nada() {
    // Arrange: conta criada direto no banco com token_version 1 (como depois de um logout); o token da
    // requisição ainda tinha a versão 0.
    let app = TestApp::new().await;
    let pool = &app.pool;
    let username = format!("t_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (username, email, password_hash, token_version) VALUES ($1, $2, 'hash-antigo', 1) RETURNING id",
    )
    .bind(&username)
    .bind(format!("{username}@email.com"))
    .fetch_one(pool)
    .await
    .unwrap();

    // Act
    let result = change_password(pool, id, 0, "hash-novo").await;

    // Assert
    assert!(matches!(result, Err(AppError::Unauthorized)));
    let (hash, version): (String, i32) =
        sqlx::query_as("SELECT password_hash, token_version FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!((hash.as_str(), version), ("hash-antigo", 1));
}

#[tokio::test]
async fn delete_de_conta_apagada_no_meio_do_caminho_da_401() {
    // A leitura do hash não encontra a conta: é o que acontece quando ela foi apagada depois da autenticação.
    let app = TestApp::new().await;

    let result = check_current_password(&app.pool, Uuid::new_v4(), "senha123").await;

    assert!(matches!(result, Err(AppError::Unauthorized)));
}
