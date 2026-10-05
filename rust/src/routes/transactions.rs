// Rotas de /transactions (o equivalente ao src/routes/transactions.ts). Todas exigem login (CurrentUser), e toda
// consulta filtra pelo user_id do token: é o que isola um usuário do outro.
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::PgPool;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::{
    auth::CurrentUser,
    dates::parse_iso_date,
    error::AppError,
    json::JsonBody,
    state::AppState,
    validation::{REQUIRED, bad_request, js_length},
};

const TYPE_ERROR: &str = "O tipo deve ser income ou expense";
const AMOUNT_ERROR: &str = "O valor deve ser um número inteiro de centavos maior que zero";
const DESCRIPTION_ERROR: &str = "A descrição deve ter entre 1 e 200 caracteres";
const DATE_ERROR: &str = "Data inválida (use AAAA-MM-DD)";
const MAX_AMOUNT_CENTS: i64 = 100_000_000_000; // R$ 1 bilhão

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list).post(create))
}

// Um registro como a API mostra (nunca o user_id). Os nomes do banco viram os da API.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicTransaction {
    pub id: Uuid,
    #[serde(rename = "type")]
    // `type` é palavra reservada em Rust: o campo se chama kind e sai como "type"
    pub kind: String,
    pub amount: i64,
    pub description: String,
    #[serde(serialize_with = "crate::dates::serialize_date")]
    pub date: Date,
    #[serde(serialize_with = "crate::dates::serialize_js_iso")]
    pub created_at: OffsetDateTime,
    #[serde(serialize_with = "crate::dates::serialize_js_iso")]
    pub updated_at: OffsetDateTime,
}

// ---- Regras dos campos (as mesmas no POST e no PATCH) ----
// Convenção do projeto: tipo JSON errado (inclusive null) → REQUIRED; tipo certo com valor fora da regra →
// a mensagem do campo. Cada função recebe o valor do campo já tirado do corpo.

pub fn parse_type(value: &Value) -> Result<String, AppError> {
    match value {
        // `as_str()` empresta o texto da String para comparar com os literais.
        Value::String(text) => match text.as_str() {
            "income" | "expense" => Ok(text.clone()),
            _ => Err(bad_request(TYPE_ERROR)),
        },
        _ => Err(bad_request(REQUIRED)),
    }
}

// Centavos inteiros, de 1 a R$ 1 bilhão. "1990" (string) é tipo errado; 19.9 é valor inválido.
// O JSON não distingue 1000 de 1000.0 (no JavaScript são o mesmo número), então 1000.0 é aceito como 1000.
pub fn parse_amount(value: &Value) -> Result<i64, AppError> {
    let Value::Number(number) = value else {
        return Err(bad_request(REQUIRED));
    };
    let cents = match number.as_i64() {
        Some(integer) => integer,
        None => {
            let float = number.as_f64().unwrap_or(f64::NAN);
            // `fract()` é a parte depois da vírgula: 19.9 → 0.9. Só aceita número inteiro dentro do teto.
            if float.fract() != 0.0 || float.abs() > MAX_AMOUNT_CENTS as f64 {
                return Err(bad_request(AMOUNT_ERROR));
            }
            float as i64
        }
    };
    if !(1..=MAX_AMOUNT_CENTS).contains(&cents) {
        return Err(bad_request(AMOUNT_ERROR));
    }
    Ok(cents)
}

pub fn parse_description(value: &Value) -> Result<String, AppError> {
    let Value::String(text) = value else {
        return Err(bad_request(REQUIRED));
    };
    let description = text.trim();
    if !(1..=200).contains(&js_length(description)) {
        return Err(bad_request(DESCRIPTION_ERROR));
    }
    Ok(description.to_string())
}

pub fn parse_date(value: &Value) -> Result<Date, AppError> {
    let Value::String(text) = value else {
        return Err(bad_request(REQUIRED));
    };
    parse_iso_date(text).ok_or_else(|| bad_request(DATE_ERROR))
}

// Campo obrigatório: ausente → REQUIRED; presente → a regra do campo.
// `impl Fn(&Value) -> Result<T, AppError>`: aceita qualquer função com essa assinatura, e o T é deduzido dela.
fn required<T>(
    body: &Map<String, Value>,
    field: &str,
    parse: impl Fn(&Value) -> Result<T, AppError>,
) -> Result<T, AppError> {
    body.get(field)
        .ok_or_else(|| bad_request(REQUIRED))
        .and_then(parse)
}

// ---- POST /transactions ----

async fn create(
    State(pool): State<PgPool>,
    current: CurrentUser,
    JsonBody(body): JsonBody,
) -> Result<(StatusCode, Json<PublicTransaction>), AppError> {
    // Ordem do schema do TS: type, amount, description, date. Campos a mais (userId, id...) são ignorados.
    let kind = required(&body, "type", parse_type)?;
    let amount = required(&body, "amount", parse_amount)?;
    let description = required(&body, "description", parse_description)?;
    let date = required(&body, "date", parse_date)?;

    // user_id vem do token, nunca do corpo. `AS "kind"`: o nome da coluna precisa bater com o campo da struct.
    let transaction = sqlx::query_as!(
        PublicTransaction,
        r#"INSERT INTO transactions (user_id, type, amount_cents, description, occurred_on)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, type AS "kind", amount_cents AS "amount", description, occurred_on AS "date",
                     created_at, updated_at"#,
        current.user.id,
        kind,
        amount,
        description,
        date,
    )
    .fetch_one(&pool)
    .await?;
    Ok((StatusCode::CREATED, Json(transaction)))
}

// ---- GET /transactions ----

#[derive(Serialize)]
struct Summary {
    income: i64,
    expense: i64,
    balance: i64,
}

#[derive(Serialize)]
struct ListResponse {
    summary: Summary,
    transactions: Vec<PublicTransaction>,
}

// Lê `from` ou `to` da query. Repetido (`?from=a&from=b`, que no Fastify vira array) → REQUIRED, como no TS.
fn query_date(pairs: &[(String, String)], name: &str) -> Result<Option<Date>, AppError> {
    // Coleta os valores com esse nome. `filter` + `map` + `collect`: o equivalente a .filter().map() de array.
    let values: Vec<&String> = pairs
        .iter()
        .filter(|(key, _)| key == name)
        .map(|(_, value)| value)
        .collect();
    match values.as_slice() {
        [] => Ok(None),
        [single] => parse_iso_date(single)
            .map(Some)
            .ok_or_else(|| bad_request(DATE_ERROR)),
        _ => Err(bad_request(REQUIRED)),
    }
}

// `Query<Vec<(String, String)>>`: a query como lista de pares, preservando os repetidos (um HashMap perderia).
// `Result<Query<...>, _>`: o próprio handler trata a falha de leitura da query (ex.: %ZZ inválido) em vez de
// deixar o axum responder em inglês.
async fn list(
    State(pool): State<PgPool>,
    current: CurrentUser,
    query: Result<Query<Vec<(String, String)>>, axum::extract::rejection::QueryRejection>,
) -> Result<Json<ListResponse>, AppError> {
    let Ok(Query(pairs)) = query else {
        return Err(bad_request(REQUIRED));
    };
    let from = query_date(&pairs, "from")?;
    let to = query_date(&pairs, "to")?;
    // "let chain" (edição 2024): o `&&` só confere `from > to` quando as duas datas vieram.
    if let (Some(from), Some(to)) = (from, to)
        && from > to
    {
        return Err(bad_request(
            "A data inicial deve ser anterior ou igual à final",
        ));
    }

    // Uma condição só para a lista e para os totais: `$2 IS NULL OR ...` desliga o filtro quando a data não veio.
    // Duas consultas sem transação (decisão registrada no AGENTS.md); `try_join!` roda as duas ao mesmo tempo.
    let list_query = sqlx::query_as!(
        PublicTransaction,
        r#"SELECT id, type AS "kind", amount_cents AS "amount", description, occurred_on AS "date",
                  created_at, updated_at
           FROM transactions
           WHERE user_id = $1 AND ($2::date IS NULL OR occurred_on >= $2) AND ($3::date IS NULL OR occurred_on <= $3)
           ORDER BY occurred_on DESC, created_at DESC"#,
        current.user.id,
        from,
        to,
    )
    .fetch_all(&pool);

    // O sum de bigint volta como numeric; `::bigint` traz de volta para inteiro. Sem linhas, o sum é NULL: o
    // coalesce troca por 0. O `!` em `AS "income!"` avisa o sqlx que a coluna nunca é nula (por causa do coalesce).
    let totals_query = sqlx::query!(
        r#"SELECT
             coalesce(sum(amount_cents) FILTER (WHERE type = 'income'), 0)::bigint AS "income!",
             coalesce(sum(amount_cents) FILTER (WHERE type = 'expense'), 0)::bigint AS "expense!"
           FROM transactions
           WHERE user_id = $1 AND ($2::date IS NULL OR occurred_on >= $2) AND ($3::date IS NULL OR occurred_on <= $3)"#,
        current.user.id,
        from,
        to,
    )
    .fetch_one(&pool);

    let (transactions, totals) = tokio::try_join!(list_query, totals_query)?;
    Ok(Json(ListResponse {
        summary: Summary {
            income: totals.income,
            expense: totals.expense,
            balance: totals.income - totals.expense,
        },
        transactions,
    }))
}
