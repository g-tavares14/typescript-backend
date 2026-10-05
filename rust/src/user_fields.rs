// Regras de username, email e senha (o equivalente ao src/lib/user-fields.ts), com as mesmas mensagens.
// Cada função recebe o texto já lido do corpo e devolve o valor normalizado, ou o AppError da regra.
use crate::error::AppError;
use crate::validation::{bad_request, js_length};

// Resposta do 409 quando username ou email já pertencem a outra conta.
pub const DUPLICATE_USER: &str = "Email ou username já cadastrado";

const USERNAME_LENGTH: &str = "O username deve ter entre 3 e 50 caracteres";
const USERNAME_CHARS: &str = "O username só pode ter letras sem acento, números e _";

// trim + minúsculas; 3 a 50 caracteres; só a-z, 0-9 e _. O tamanho é conferido antes dos caracteres, então um
// username curto recebe a mensagem de tamanho (como no Zod).
pub fn parse_username(raw: &str) -> Result<String, AppError> {
    // `String` (e não `&str`): o valor normalizado é um texto novo, que a função cria e devolve (passa a posse).
    let username = raw.trim().to_lowercase();
    if !(3..=50).contains(&js_length(&username)) {
        return Err(bad_request(USERNAME_LENGTH));
    }
    // `.chars().all(...)`: percorre as letras e confere a condição em todas (o regex ^[a-z0-9_]+$ do TS).
    if !username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(bad_request(USERNAME_CHARS));
    }
    Ok(username)
}

// trim + minúsculas e o mesmo formato do z.email() do Zod 4:
// ^(?:[A-Za-z0-9_'+\-]+\.)*[A-Za-z0-9_'+\-]*[A-Za-z0-9_+-]@(?:[A-Za-z0-9][A-Za-z0-9\-]*\.)+[A-Za-z]{2,}$
// Escrito à mão (sem crate de regex), parte por parte.
pub fn parse_email(raw: &str) -> Result<String, AppError> {
    let email = raw.trim().to_lowercase();
    if is_valid_email(&email) {
        Ok(email)
    } else {
        Err(bad_request("Email inválido"))
    }
}

fn is_valid_email(email: &str) -> bool {
    // Exatamente um @. `split_once` devolve Option<(&str, &str)>: None se não houver @.
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    is_valid_local_part(local) && is_valid_domain(domain)
}

fn is_local_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '\'' | '+' | '-')
}

// Parte antes do @: pedaços separados por ponto, nenhum vazio (sem ponto no início, no fim ou dois seguidos),
// só com os caracteres permitidos, e o último caractere não pode ser apóstrofo.
fn is_valid_local_part(local: &str) -> bool {
    !local.is_empty()
        && local
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(is_local_char))
        && !local.ends_with('\'')
}

// Domínio: pelo menos dois rótulos separados por ponto. Os rótulos começam com letra ou número e seguem com
// letra, número ou hífen; o último (o "com", "br"...) tem só letras e pelo menos 2.
fn is_valid_domain(domain: &str) -> bool {
    // `rsplit_once` separa no ÚLTIMO ponto: ("email.com", "br") em "email.com.br".
    let Some((labels, tld)) = domain.rsplit_once('.') else {
        return false;
    };
    let tld_ok = tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic());
    let labels_ok = labels.split('.').all(|label| {
        let mut chars = label.chars();
        // `chars.next()` tira o primeiro caractere; `chars.all(...)` confere o resto.
        matches!(chars.next(), Some(first) if first.is_ascii_alphanumeric())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
    });
    tld_ok && labels_ok
}

// Senha nova (cadastro e, depois, troca de senha): mínimo 8 caracteres, contados como no JavaScript.
pub fn check_new_password(password: &str) -> Result<(), AppError> {
    if js_length(password) < 8 {
        return Err(bad_request("A senha deve ter no mínimo 8 caracteres"));
    }
    Ok(())
}

// Testes unitários ficam no próprio arquivo: `#[cfg(test)]` faz o módulo existir só no `cargo test`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_validos() {
        for email in [
            "a@b.co",
            "joao.silva@email.com.br",
            "o'neil+tag@x-y.io",
            "a_b-c@1a.com",
        ] {
            assert!(is_valid_email(email), "{email}");
        }
    }

    #[test]
    fn emails_invalidos() {
        let invalidos = [
            "nao-e-email",
            "@b.com",
            "a@",
            "a@b",
            "a@b.c",
            "a..b@c.com",
            ".a@c.com",
            "a.@c.com",
            "a'@c.com",
            "a@-b.com",
            "a@b..com",
            "a@b.c0m",
            "a@@b.com",
            "a b@c.com",
            "jõao@c.com",
        ];
        for email in invalidos {
            assert!(!is_valid_email(email), "{email}");
        }
    }

    #[test]
    fn username_normaliza_e_confere_tamanho_antes_dos_caracteres() {
        assert_eq!(parse_username("  Joao_1 ").unwrap(), "joao_1");
        assert!(
            matches!(parse_username("jõ"), Err(AppError::BadRequest(m)) if m == USERNAME_LENGTH)
        );
        assert!(
            matches!(parse_username("joão"), Err(AppError::BadRequest(m)) if m == USERNAME_CHARS)
        );
    }
}
