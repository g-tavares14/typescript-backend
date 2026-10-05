// Geração e verificação do JWT (o equivalente ao src/lib/token.ts). HS256 com o JWT_SECRET: tokens do TS valem
// aqui e vice-versa.
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

pub const ACCESS_TOKEN_TTL_SECONDS: u64 = 60 * 60; // 1 hora

// Conteúdo do token. Sem role (ela mudaria no banco e o token ficaria desatualizado).
// `ver` é a versão do token do usuário: o token só vale se for igual a users.token_version.
#[derive(Serialize, Deserialize)]
struct Claims {
    sub: Uuid, // dono do token; um "sub" que não é UUID falha na leitura (como o z.uuid() do TS)
    ver: u32,  // u32: inteiro >= 0; negativo, fração ou ausente falham na leitura
    iat: u64,  // emitido em (segundos desde 1970)
    exp: u64,  // expira em
}

// Chaves derivadas do segredo, criadas uma vez na inicialização.
pub struct TokenKeys {
    encoding: EncodingKey,
    decoding: DecodingKey,
}

// O que um token válido diz: de quem é e qual versão.
pub struct VerifiedToken {
    pub user_id: Uuid,
    pub token_version: i32,
}

impl TokenKeys {
    pub fn new(secret: &str) -> Self {
        TokenKeys {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn create_access_token(
        &self,
        user_id: Uuid,
        token_version: i32,
    ) -> Result<String, AppError> {
        let now = now_seconds();
        let claims = Claims {
            sub: user_id,
            // `try_from`: i32 (coluna do banco) para u32 só se não for negativo. A coluna nunca é negativa.
            ver: u32::try_from(token_version).map_err(AppError::internal)?,
            iat: now,
            exp: now + ACCESS_TOKEN_TTL_SECONDS,
        };
        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding).map_err(AppError::internal)
    }

    // Confere assinatura, algoritmo, expiração e formato. Se a versão ainda é a atual, quem decide é o banco.
    // `None` para qualquer token inválido: o motivo não interessa ao cliente (sempre o mesmo 401).
    pub fn verify_access_token(&self, token: &str) -> Option<VerifiedToken> {
        let mut validation = Validation::new(Algorithm::HS256); // só HS256: barra o "alg: none" e a troca de algoritmo
        validation.set_required_spec_claims(&["exp", "sub"]); // sem "exp", o token valeria para sempre
        validation.leeway = 0; // o padrão da crate aceita 60 s depois do exp; o jose do TS aceita 0
        let data = decode::<Claims>(token, &self.decoding, &validation).ok()?;
        Some(VerifiedToken {
            user_id: data.claims.sub,
            token_version: i32::try_from(data.claims.ver).ok()?,
        })
    }
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("o relógio do sistema está antes de 1970")
        .as_secs()
}
