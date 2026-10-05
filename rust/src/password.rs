// Hash e verificação de senha com argon2id (o equivalente ao src/lib/password.ts).
use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher, PasswordVerifier},
};

use crate::error::AppError;

// Os mesmos parâmetros do node-argon2 do TS: 64 MiB, 3 passadas, 4 vias, 32 bytes. O padrão da crate é mais
// leve (19 MiB, 2 passadas, 1 via); com os mesmos valores, uma senha custa igual nos dois servidores.
fn hasher() -> Argon2<'static> {
    let params =
        Params::new(65536, 3, 4, Some(32)).expect("parâmetros fixos do argon2 são válidos");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

// O argon2 ocupa a CPU por dezenas de milissegundos. `spawn_blocking` roda o cálculo numa thread separada,
// para não travar as threads do tokio que atendem as outras requisições (no Node, a lib já faz isso em C++).
// `move` passa a posse da senha para a closure, que vai rodar em outra thread.
pub async fn hash_password(password: String) -> Result<String, AppError> {
    tokio::task::spawn_blocking(move || {
        hasher()
            .hash_password(password.as_bytes()) // sal aleatório gerado pela crate (feature getrandom)
            .map(|hash| hash.to_string()) // formato PHC: $argon2id$v=19$m=65536,t=3,p=4$sal$hash
            .map_err(|error| AppError::internal(error.to_string()))
    })
    .await
    .map_err(AppError::internal)? // a thread entrou em pânico
}

// Verifica a senha contra o hash salvo. Os parâmetros vêm do próprio hash, então hashes gerados pelo TS valem.
// Hash corrompido conta como senha errada (false), sem derrubar a requisição.
pub async fn verify_password(hash: String, password: String) -> Result<bool, AppError> {
    tokio::task::spawn_blocking(move || {
        hasher()
            .verify_password(password.as_bytes(), hash.as_str())
            .is_ok()
    })
    .await
    .map_err(AppError::internal)
}
