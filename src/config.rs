// Variáveis de ambiente (o equivalente ao src/config.ts). Lidas uma vez na inicialização: se faltar algo
// essencial, o servidor nem sobe (fail fast).
use std::env;

// `#[derive(Debug)]` gera a formatação para depuração (`{:?}`). Os campos são `pub` para o main.rs ler.
// Atenção: com Debug, `println!("{config:?}")` mostraria o segredo. Nunca logar a config inteira.
#[derive(Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub port: u16,
}

// Erro de configuração: só uma mensagem. `Result<T, String>` é o jeito mais simples de dizer
// "deu certo com um T, ou deu errado com esta mensagem".
fn require_env(name: &str) -> Result<String, String> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => Err(format!("Variável de ambiente {name} não definida")),
    }
}

fn require_jwt_secret() -> Result<String, String> {
    // O `?` devolve o erro na hora, se houver; senão, tira o valor de dentro do Ok.
    let secret = require_env("JWT_SECRET")?;
    // Um segredo curto (ou o valor de exemplo do .env.example) pode ser adivinhado por força bruta.
    if secret.len() < 32 || secret.starts_with("troque-por") {
        return Err(
            "JWT_SECRET deve ser um valor aleatório com pelo menos 32 caracteres".to_string(),
        );
    }
    Ok(secret)
}

impl Config {
    // Função associada (sem `self`), o equivalente a um método estático: `Config::from_env()`.
    pub fn from_env() -> Result<Config, String> {
        // Porta 3001 por padrão, para rodar ao lado do servidor TypeScript (3000). `RUST_PORT` e não `PORT`,
        // porque o .env é compartilhado com o TS.
        let port = match env::var("RUST_PORT") {
            Ok(value) => value
                .parse()
                .map_err(|_| format!("RUST_PORT inválida: {value}"))?,
            Err(_) => 3001,
        };
        Ok(Config {
            database_url: require_env("DATABASE_URL")?,
            jwt_secret: require_jwt_secret()?,
            port,
        })
    }
}
