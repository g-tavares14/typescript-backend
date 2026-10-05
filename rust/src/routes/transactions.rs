// Rotas de /transactions (o equivalente ao src/routes/transactions.ts). Todas exigem login (CurrentUser), e toda
// consulta filtra pelo user_id do token: é o que isola um usuário do outro.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch},
};
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::PgPool;
use time::Date;
use uuid::Uuid;

use crate::{
    http::auth::CurrentUser,
    http::error::AppError,
    http::json::JsonBody,
    models::transaction::PublicTransaction,
    state::AppState,
    validation::transaction_fields::{
        parse_amount, parse_date, parse_date_text, parse_description, parse_type,
    },
    validation::{REQUIRED, bad_request},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{id}", patch(update).delete(remove))
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
        [single] => parse_date_text(single).map(Some),
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

// ---- PATCH e DELETE /transactions/{id} ----

// Mesma resposta para id inexistente, id inválido e registro de outro usuário: um 403 confirmaria que o id existe.
const NOT_FOUND: &str = "Registro não encontrado";

// O {id} da URL. Só o formato com hífens (36 caracteres), como o z.uuid() do TS: o Uuid::parse_str também aceitaria
// a forma sem hífens e com chaves. Id inválido vira 404 sem ir ao banco.
fn parse_id(text: &str) -> Result<Uuid, AppError> {
    if text.len() != 36 {
        return Err(AppError::NotFoundMessage(NOT_FOUND));
    }
    Uuid::parse_str(text).map_err(|_| AppError::NotFoundMessage(NOT_FOUND))
}

// Campo opcional do PATCH: ausente → None; presente → a mesma regra do POST (null ou tipo errado → REQUIRED).
fn optional<T>(
    body: &Map<String, Value>,
    field: &str,
    parse: impl Fn(&Value) -> Result<T, AppError>,
) -> Result<Option<T>, AppError> {
    // `Option::map` aplica a regra se o campo veio; `transpose` vira Option<Result> em Result<Option>.
    body.get(field).map(parse).transpose()
}

// Os extractors vêm nesta ordem: CurrentUser (401), Path (texto, nunca falha), JsonBody (erros do corpo).
// O id é conferido dentro do handler, então a ordem dos erros é a da spec: 401 → corpo → :id → campos → 404.
async fn update(
    State(pool): State<PgPool>,
    current: CurrentUser,
    Path(id): Path<String>,
    JsonBody(body): JsonBody,
) -> Result<Json<PublicTransaction>, AppError> {
    let id = parse_id(&id)?;
    let kind = optional(&body, "type", parse_type)?;
    let amount = optional(&body, "amount", parse_amount)?;
    let description = optional(&body, "description", parse_description)?;
    let date = optional(&body, "date", parse_date)?;
    if kind.is_none() && amount.is_none() && description.is_none() && date.is_none() {
        return Err(bad_request("Envie ao menos um campo para alterar"));
    }

    // COALESCE: campo não enviado (NULL) mantém o valor atual. updated_at sempre muda, com o now() do banco.
    // id e user_id na mesma condição, numa consulta só: registro de outro usuário = 0 linhas = 404.
    sqlx::query_as!(
        PublicTransaction,
        r#"UPDATE transactions
           SET type = COALESCE($3, type), amount_cents = COALESCE($4, amount_cents),
               description = COALESCE($5, description), occurred_on = COALESCE($6, occurred_on),
               updated_at = now()
           WHERE id = $1 AND user_id = $2
           RETURNING id, type AS "kind", amount_cents AS "amount", description, occurred_on AS "date",
                     created_at, updated_at"#,
        id,
        current.user.id,
        kind,
        amount,
        description,
        date,
    )
    .fetch_optional(&pool)
    .await?
    .map(Json)
    .ok_or(AppError::NotFoundMessage(NOT_FOUND))
}

// Exclusão definitiva. Sem corpo.
async fn remove(
    State(pool): State<PgPool>,
    current: CurrentUser,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id = parse_id(&id)?;
    let deleted = sqlx::query!(
        "DELETE FROM transactions WHERE id = $1 AND user_id = $2",
        id,
        current.user.id
    )
    .execute(&pool)
    .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFoundMessage(NOT_FOUND));
    }
    Ok(StatusCode::NO_CONTENT)
}
