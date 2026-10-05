// POST e GET /transactions (equivalente ao test/transactions.test.ts).
mod common;

use axum::http::StatusCode;
use common::{TestApp, TestResponse, assert_unauthorized};
use serde_json::{Value, json};

const REQUIRED: &str = "Campo obrigatório ausente ou inválido";
const TYPE_ERROR: &str = "O tipo deve ser income ou expense";
const AMOUNT_ERROR: &str = "O valor deve ser um número inteiro de centavos maior que zero";
const DESCRIPTION_ERROR: &str = "A descrição deve ter entre 1 e 200 caracteres";
const DATE_ERROR: &str = "Data inválida (use AAAA-MM-DD)";
const RANGE_ERROR: &str = "A data inicial deve ser anterior ou igual à final";

fn valid_body() -> Value {
    json!({ "type": "expense", "amount": 1990, "description": "Almoço", "date": "2026-09-29" })
}

// O corpo válido com campos trocados; `null` aqui também é um valor (o `{ ...validBody, campo: null }` do TS).
fn body_with(changes: Value) -> Value {
    let mut body = valid_body();
    for (key, value) in changes.as_object().unwrap() {
        body[key] = value.clone();
    }
    body
}

// O corpo válido sem um campo (o `campo: undefined` do TS, que o JSON.stringify remove).
fn body_without(field: &str) -> Value {
    let mut body = valid_body();
    body.as_object_mut().unwrap().remove(field);
    body
}

fn transaction(kind: &str, amount: i64, description: &str, date: &str) -> Value {
    json!({ "type": kind, "amount": amount, "description": description, "date": date })
}

fn maria() -> Value {
    json!({ "username": "maria", "email": "maria@email.com", "password": "senha123" })
}

async fn post(app: &TestApp, token: &str, body: &Value) -> TestResponse {
    app.post("/transactions")
        .bearer(token)
        .json(body)
        .send()
        .await
}

async fn get(app: &TestApp, token: &str, query: &str) -> TestResponse {
    app.get(&format!("/transactions{query}"))
        .bearer(token)
        .send()
        .await
}

// Cria os registros pelo POST de verdade, em ordem: o createdAt de cada um é posterior ao do anterior.
async fn create_all(app: &TestApp, token: &str, bodies: &[Value]) {
    for body in bodies {
        let response = post(app, token, body).await;
        assert_eq!(response.status, StatusCode::CREATED, "{}", response.body);
    }
}

async fn count(app: &TestApp) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM transactions")
        .fetch_one(&app.pool)
        .await
        .unwrap()
}

fn descriptions(response: &TestResponse) -> Vec<String> {
    response.json()["transactions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["description"].as_str().unwrap().to_string())
        .collect()
}

// ---- POST /transactions ----

#[tokio::test]
async fn post_responde_201_com_o_registro_criado_sem_expor_o_user_id() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = post(&app, &token, &valid_body()).await;

    assert_eq!(response.status, StatusCode::CREATED);
    let body = response.json();
    for field in ["id", "createdAt", "updatedAt"] {
        assert!(body[field].is_string(), "{field}");
    }
    assert_eq!(
        body,
        json!({
            "id": body["id"],
            "type": "expense",
            "amount": 1990,
            "description": "Almoço",
            "date": "2026-09-29",
            "createdAt": body["createdAt"],
            "updatedAt": body["updatedAt"],
        })
    );
}

#[tokio::test]
async fn registro_recem_criado_tem_updated_at_igual_ao_created_at() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let body = post(&app, &token, &valid_body()).await.json();

    // created_at e updated_at usam o now() da mesma instrução, então saem idênticos.
    assert!(body["updatedAt"].is_string());
    assert_eq!(body["updatedAt"], body["createdAt"]);
}

#[tokio::test]
async fn post_salva_o_registro_no_banco_com_o_user_id_do_token() {
    let app = TestApp::new().await;
    let user_id = app.register(common::default_user()).await["id"].clone();
    let token = app
        .login(common::DEFAULT_EMAIL, common::DEFAULT_PASSWORD)
        .await;

    let id = post(&app, &token, &valid_body()).await.json()["id"].clone();

    let rows: Vec<(String, String, String, i64, String, String)> = sqlx::query_as(
        "SELECT id::text, user_id::text, type, amount_cents, description, occurred_on::text FROM transactions",
    )
    .fetch_all(&app.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    let (row_id, row_user, kind, amount, description, date) = &rows[0];
    assert_eq!((json!(row_id), json!(row_user)), (id, user_id));
    assert_eq!(
        (kind.as_str(), *amount, description.as_str(), date.as_str()),
        ("expense", 1990, "Almoço", "2026-09-29")
    );
}

#[tokio::test]
async fn post_ignora_user_id_id_e_created_at_enviados_no_corpo() {
    // Arrange: dois usuários; o A tenta criar um registro em nome do B.
    let app = TestApp::new().await;
    let a = app.register(common::default_user()).await["id"].clone();
    let b = app.register(maria()).await["id"].clone();
    let token_a = app
        .login(common::DEFAULT_EMAIL, common::DEFAULT_PASSWORD)
        .await;
    let forged_id = "00000000-0000-4000-8000-000000000000";

    let response = post(
        &app,
        &token_a,
        &body_with(
            json!({ "userId": b, "id": forged_id, "createdAt": "2000-01-01T00:00:00.000Z" }),
        ),
    )
    .await;

    assert_eq!(response.status, StatusCode::CREATED);
    let body = response.json();
    assert_ne!(body["id"], forged_id);
    assert_ne!(body["createdAt"], "2000-01-01T00:00:00.000Z");
    let owner: String =
        sqlx::query_scalar("SELECT user_id::text FROM transactions WHERE id::text = $1")
            .bind(body["id"].as_str().unwrap())
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(json!(owner), a);
    assert_eq!(count(&app).await, 1);
}

#[tokio::test]
async fn post_com_campo_ausente_ou_tipo_errado_da_400_required_e_nao_grava_nada() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let cases = [
        ("type ausente", body_without("type")),
        ("amount ausente", body_without("amount")),
        ("description ausente", body_without("description")),
        ("date ausente", body_without("date")),
        ("corpo vazio", json!({})),
        ("type numérico", body_with(json!({ "type": 123 }))),
        ("type nulo", body_with(json!({ "type": null }))),
        ("amount em string", body_with(json!({ "amount": "1990" }))),
        ("amount nulo", body_with(json!({ "amount": null }))),
        (
            "description numérica",
            body_with(json!({ "description": 5 })),
        ),
        ("date numérica", body_with(json!({ "date": 20260929 }))),
    ];

    for (case, body) in cases {
        let response = post(&app, &token, &body).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": REQUIRED }), "{case}");
    }
    assert_eq!(count(&app).await, 0);
}

#[tokio::test]
async fn post_com_valor_fora_da_regra_da_400_com_a_mensagem_do_campo_e_nao_grava_nada() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let cases = [
        (
            "type fora de income/expense",
            json!({ "type": "foo" }),
            TYPE_ERROR,
        ),
        (
            "type com maiúscula",
            json!({ "type": "Income" }),
            TYPE_ERROR,
        ),
        ("amount 0", json!({ "amount": 0 }), AMOUNT_ERROR),
        ("amount negativo", json!({ "amount": -1 }), AMOUNT_ERROR),
        (
            "amount em reais (19.9)",
            json!({ "amount": 19.9 }),
            AMOUNT_ERROR,
        ),
        (
            "amount acima de R$ 1 bilhão",
            json!({ "amount": 100_000_000_001_i64 }),
            AMOUNT_ERROR,
        ),
        (
            "description vazia",
            json!({ "description": "" }),
            DESCRIPTION_ERROR,
        ),
        (
            "description só com espaços",
            json!({ "description": "   " }),
            DESCRIPTION_ERROR,
        ),
        (
            "description com 201 caracteres",
            json!({ "description": "a".repeat(201) }),
            DESCRIPTION_ERROR,
        ),
        (
            "date inexistente (2026-02-30)",
            json!({ "date": "2026-02-30" }),
            DATE_ERROR,
        ),
        (
            "date no formato brasileiro",
            json!({ "date": "29/09/2026" }),
            DATE_ERROR,
        ),
    ];

    for (case, changes, message) in cases {
        let response = post(&app, &token, &body_with(changes)).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": message }), "{case}");
    }
    assert_eq!(count(&app).await, 0);
}

#[tokio::test]
async fn post_aceita_os_limites() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let cases = [
        (
            "amount igual ao teto (R$ 1 bilhão)",
            json!({ "amount": 100_000_000_000_i64 }),
        ),
        ("amount 1 centavo", json!({ "amount": 1 })),
        (
            "description com exatamente 200 caracteres",
            json!({ "description": "a".repeat(200) }),
        ),
        ("date no futuro", json!({ "date": "2099-12-31" })),
        ("type income", json!({ "type": "income" })),
    ];

    for (case, changes) in cases {
        let response = post(&app, &token, &body_with(changes.clone())).await;
        assert_eq!(response.status, StatusCode::CREATED, "{case}");
        let body = response.json();
        for (key, value) in changes.as_object().unwrap() {
            assert_eq!(&body[key], value, "{case}");
        }
    }
    assert_eq!(count(&app).await, 5);
}

#[tokio::test]
async fn post_aplica_trim_na_descricao_antes_de_validar_o_tamanho_e_antes_de_salvar() {
    // 200 caracteres úteis + espaços nas pontas só passam se o trim vier antes do limite de 200.
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let description = "a".repeat(200);

    for (raw, saved) in [
        (format!("  {description}  "), description.as_str()),
        ("  Almoço  ".to_string(), "Almoço"),
    ] {
        let response = post(&app, &token, &body_with(json!({ "description": raw }))).await;
        assert_eq!(response.status, StatusCode::CREATED);
        assert_eq!(response.json()["description"], saved);
        let row: String =
            sqlx::query_scalar("SELECT description FROM transactions WHERE id::text = $1")
                .bind(response.json()["id"].as_str().unwrap())
                .fetch_one(&app.pool)
                .await
                .unwrap();
        assert_eq!(row, saved);
    }
}

#[tokio::test]
async fn post_responde_401_sem_token_com_token_invalido_e_com_token_revogado_e_nao_grava_nada() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    app.post("/auth/logout").bearer(&token).send().await;

    assert_unauthorized(&app.post("/transactions").json(&valid_body()).send().await);
    assert_unauthorized(&post(&app, "nao-e-um-jwt", &valid_body()).await);
    assert_unauthorized(&post(&app, &token, &valid_body()).await);
    assert_eq!(count(&app).await, 0);
}

// ---- GET /transactions ----

#[tokio::test]
async fn get_sem_registros_da_lista_vazia_e_totais_0() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    let response = get(&app, &token, "").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.json(),
        json!({ "summary": { "income": 0, "expense": 0, "balance": 0 }, "transactions": [] })
    );
}

#[tokio::test]
async fn get_soma_entradas_e_saidas_e_devolve_os_totais_como_numero() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let cases = [
        // (registros, totais esperados)
        (
            vec![
                transaction("income", 500000, "Salário", "2026-09-05"),
                transaction("income", 25000, "Freela", "2026-09-10"),
                transaction("expense", 120000, "Aluguel", "2026-09-06"),
            ],
            json!({ "income": 525000, "expense": 120000, "balance": 405000 }),
        ),
        // Saídas maiores: balance negativo (somando com os registros do caso anterior).
        (
            vec![transaction("expense", 500000, "Carro", "2026-09-07")],
            json!({ "income": 525000, "expense": 620000, "balance": -95000 }),
        ),
    ];

    for (bodies, summary) in cases {
        create_all(&app, &token, &bodies).await;
        // O `json!` compara número com número: "525000" (string) não seria igual.
        assert_eq!(get(&app, &token, "").await.json()["summary"], summary);
    }
}

#[tokio::test]
async fn get_so_com_entradas_expense_e_0_e_nao_null() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    create_all(
        &app,
        &token,
        &[transaction("income", 7000, "Venda", "2026-09-05")],
    )
    .await;

    assert_eq!(
        get(&app, &token, "").await.json()["summary"],
        json!({ "income": 7000, "expense": 0, "balance": 7000 })
    );
}

#[tokio::test]
async fn cada_registro_da_lista_tem_o_formato_publico_e_updated_at_igual_ao_created_at() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    create_all(
        &app,
        &token,
        &[valid_body(), transaction("income", 100, "Um", "2026-09-01")],
    )
    .await;

    let list = get(&app, &token, "").await.json()["transactions"].clone();

    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 2);
    for item in list {
        let mut keys: Vec<&String> = item.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "amount",
                "createdAt",
                "date",
                "description",
                "id",
                "type",
                "updatedAt"
            ]
        );
        assert_eq!(item["updatedAt"], item["createdAt"]);
    }
}

#[tokio::test]
async fn get_ordena_por_data_mais_recente_e_desempata_por_created_at_mais_recente() {
    // Inseridos fora de ordem; "B" e "C" têm a mesma data, e "C" foi criado depois de "B".
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    create_all(
        &app,
        &token,
        &[
            transaction("expense", 100, "A antigo", "2026-09-01"),
            transaction("expense", 200, "B empate", "2026-09-20"),
            transaction("expense", 300, "C empate", "2026-09-20"),
            transaction("income", 400, "D recente", "2026-10-02"),
        ],
    )
    .await;

    let response = get(&app, &token, "").await;

    assert_eq!(
        descriptions(&response),
        ["D recente", "C empate", "B empate", "A antigo"]
    );
}

#[tokio::test]
async fn isolamento_o_usuario_b_nao_ve_nem_soma_os_registros_do_a() {
    let app = TestApp::new().await;
    let token_a = app.register_and_login().await;
    let token_b = app.register_and_login_as(maria()).await;
    // Antes de qualquer registro do B: lista vazia, mesmo com registros do A.
    create_all(
        &app,
        &token_a,
        &[transaction("income", 500000, "Salário do A", "2026-09-05")],
    )
    .await;
    assert_eq!(
        get(&app, &token_b, "").await.json(),
        json!({ "summary": { "income": 0, "expense": 0, "balance": 0 }, "transactions": [] })
    );
    create_all(
        &app,
        &token_a,
        &[transaction("expense", 1000, "Café do A", "2026-09-06")],
    )
    .await;
    create_all(
        &app,
        &token_b,
        &[transaction("expense", 2500, "Cinema do B", "2026-09-07")],
    )
    .await;

    let response_a = get(&app, &token_a, "").await;
    let response_b = get(&app, &token_b, "").await;

    assert_eq!(
        response_a.json()["summary"],
        json!({ "income": 500000, "expense": 1000, "balance": 499000 })
    );
    assert_eq!(descriptions(&response_a), ["Café do A", "Salário do A"]);
    assert_eq!(
        response_b.json()["summary"],
        json!({ "income": 0, "expense": 2500, "balance": -2500 })
    );
    assert_eq!(descriptions(&response_b), ["Cinema do B"]);
}

// ---- Filtro from/to ----

// Um registro logo antes, dois exatamente nos limites, um no meio e um logo depois de setembro.
async fn user_with_boundary_transactions(app: &TestApp) -> String {
    let token = app.register_and_login().await;
    create_all(
        app,
        &token,
        &[
            transaction("income", 1000, "Antes", "2026-08-31"),
            transaction("income", 2000, "Limite inicial", "2026-09-01"),
            transaction("expense", 300, "Meio", "2026-09-15"),
            transaction("expense", 40, "Limite final", "2026-09-30"),
            transaction("income", 5, "Depois", "2026-10-01"),
        ],
    )
    .await;
    token
}

#[tokio::test]
async fn filtro_from_e_to_sao_inclusivos_e_os_totais_usam_o_mesmo_filtro_da_lista() {
    let app = TestApp::new().await;
    let token = user_with_boundary_transactions(&app).await;
    let cases: [(&str, &[&str], Value); 5] = [
        (
            "?from=2026-09-01&to=2026-09-30",
            &["Limite final", "Meio", "Limite inicial"],
            json!({ "income": 2000, "expense": 340, "balance": 1660 }),
        ),
        (
            "?from=2026-09-30",
            &["Depois", "Limite final"],
            json!({ "income": 5, "expense": 40, "balance": -35 }),
        ),
        (
            "?to=2026-09-01",
            &["Limite inicial", "Antes"],
            json!({ "income": 3000, "expense": 0, "balance": 3000 }),
        ),
        (
            "?from=2026-09-15&to=2026-09-15",
            &["Meio"],
            json!({ "income": 0, "expense": 300, "balance": -300 }),
        ),
        (
            "?from=2027-01-01&to=2027-01-31",
            &[],
            json!({ "income": 0, "expense": 0, "balance": 0 }),
        ),
    ];

    for (query, expected, summary) in cases {
        let response = get(&app, &token, query).await;
        assert_eq!(response.status, StatusCode::OK, "{query}");
        assert_eq!(descriptions(&response), expected, "{query}");
        assert_eq!(response.json()["summary"], summary, "{query}");
    }
}

#[tokio::test]
async fn filtro_com_isolamento_o_usuario_b_continua_sem_ver_os_registros_do_a() {
    let app = TestApp::new().await;
    let token_a = app.register_and_login().await;
    let token_b = app.register_and_login_as(maria()).await;
    create_all(
        &app,
        &token_a,
        &[transaction("income", 90000, "Do A", "2026-09-10")],
    )
    .await;
    create_all(
        &app,
        &token_b,
        &[transaction("expense", 700, "Do B", "2026-09-12")],
    )
    .await;

    let response = get(&app, &token_b, "?from=2026-09-01&to=2026-09-30").await;

    assert_eq!(descriptions(&response), ["Do B"]);
    assert_eq!(
        response.json()["summary"],
        json!({ "income": 0, "expense": 700, "balance": -700 })
    );
}

#[tokio::test]
async fn filtro_invalido_da_400() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let cases = [
        ("from com dia inexistente", "?from=2026-02-30", DATE_ERROR),
        ("to com formato errado", "?to=29/09/2026", DATE_ERROR),
        ("from vazio", "?from=", DATE_ERROR),
        (
            "from com hora junto",
            "?from=2026-09-01T00:00:00Z",
            DATE_ERROR,
        ),
        (
            "to inválido com from válido",
            "?from=2026-09-30&to=lixo",
            DATE_ERROR,
        ),
        (
            "from repetido",
            "?from=2026-09-01&from=2026-09-02",
            REQUIRED,
        ),
        ("to repetido", "?to=2026-09-01&to=2026-09-02", REQUIRED),
        (
            "from depois de to",
            "?from=2026-09-30&to=2026-09-01",
            RANGE_ERROR,
        ),
    ];

    for (case, query, message) in cases {
        let response = get(&app, &token, query).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": message }), "{case}");
    }
}

#[tokio::test]
async fn get_responde_401_sem_token_com_token_invalido_e_com_token_revogado() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    app.post("/auth/logout").bearer(&token).send().await;

    // Query inválida sem token: a autenticação vem antes da validação.
    assert_unauthorized(&app.get("/transactions?from=lixo").send().await);
    assert_unauthorized(&app.get("/transactions").send().await);
    assert_unauthorized(&get(&app, "nao-e-um-jwt", "").await);
    assert_unauthorized(&get(&app, &token, "").await);
}
