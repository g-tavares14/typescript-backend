// PATCH e DELETE /transactions/:id (equivalente ao test/transactions-update-delete.test.ts).
mod common;

use axum::http::StatusCode;
use common::{DEFAULT_EMAIL, DEFAULT_PASSWORD, TestApp, TestResponse, assert_unauthorized};
use serde_json::{Value, json};

const NOT_FOUND: &str = "Registro não encontrado";
const INVALID_BODY: &str = "Corpo da requisição inválido: envie um objeto JSON";
const UNSUPPORTED: &str = "Tipo de conteúdo não suportado (use application/json)";
const REQUIRED: &str = "Campo obrigatório ausente ou inválido";
const NO_FIELDS: &str = "Envie ao menos um campo para alterar";
const TYPE_ERROR: &str = "O tipo deve ser income ou expense";
const AMOUNT_ERROR: &str = "O valor deve ser um número inteiro de centavos maior que zero";
const DESCRIPTION_ERROR: &str = "A descrição deve ter entre 1 e 200 caracteres";
const DATE_ERROR: &str = "Data inválida (use AAAA-MM-DD)";
const MISSING_ID: &str = "00000000-0000-4000-8000-000000000000";
// POST e PATCH no mesmo milissegundo deixariam a comparação de updatedAt instável: as datas são recuadas no banco.
const PAST: &str = "2026-01-01T00:00:00.000Z";

fn valid_body() -> Value {
    json!({ "type": "expense", "amount": 1990, "description": "Almoço", "date": "2026-09-29" })
}

fn maria() -> Value {
    json!({ "username": "maria", "email": "maria@email.com", "password": "senha123" })
}

async fn create(app: &TestApp, token: &str, body: Value) -> String {
    let response = app
        .post("/transactions")
        .bearer(token)
        .json(&body)
        .send()
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.body);
    response.json()["id"].as_str().unwrap().to_string()
}

async fn create_backdated(app: &TestApp, token: &str) -> String {
    let id = create(app, token, valid_body()).await;
    sqlx::query("UPDATE transactions SET created_at = $1::timestamptz, updated_at = $1::timestamptz WHERE id::text = $2")
        .bind(PAST)
        .bind(&id)
        .execute(&app.pool)
        .await
        .unwrap();
    id
}

async fn patch(app: &TestApp, token: &str, id: &str, body: Value) -> TestResponse {
    app.patch(&format!("/transactions/{id}"))
        .bearer(token)
        .json(&body)
        .send()
        .await
}

async fn delete(app: &TestApp, token: &str, id: &str) -> TestResponse {
    app.delete(&format!("/transactions/{id}"))
        .bearer(token)
        .send()
        .await
}

async fn list(app: &TestApp, token: &str) -> Value {
    app.get("/transactions").bearer(token).send().await.json()
}

fn ids(list: &Value) -> Vec<String> {
    list["transactions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect()
}

fn assert_not_found(response: &TestResponse) {
    assert_eq!(response.status, StatusCode::NOT_FOUND, "{}", response.body);
    assert_eq!(response.json(), json!({ "error": NOT_FOUND }));
}

// O updatedAt (texto ISO) é depois de PAST. Datas ISO no mesmo formato e fuso comparam certo como texto.
fn assert_after_past(updated_at: &Value) {
    let text = updated_at.as_str().unwrap();
    assert_eq!(text.len(), PAST.len(), "{text}");
    assert!(text > PAST, "{text}");
}

// ---- DELETE /transactions/:id ----

#[tokio::test]
async fn delete_do_proprio_registro_da_204_com_corpo_vazio() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let id = create(&app, &token, valid_body()).await;

    let response = delete(&app, &token, &id).await;

    assert_eq!(response.status, StatusCode::NO_CONTENT);
    assert_eq!(response.body, "");
}

#[tokio::test]
async fn o_registro_excluido_some_do_get_e_dos_totais() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let expense = create(&app, &token, valid_body()).await;
    let mut income = valid_body();
    income["type"] = json!("income");
    income["amount"] = json!(5000);
    let income = create(&app, &token, income).await;

    delete(&app, &token, &expense).await;

    let list = list(&app, &token).await;
    assert_eq!(ids(&list), [income]);
    assert_eq!(
        list["summary"],
        json!({ "income": 5000, "expense": 0, "balance": 5000 })
    );
}

#[tokio::test]
async fn delete_de_id_inexistente_que_nao_e_uuid_ou_ja_excluido_da_404() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let id = create(&app, &token, valid_body()).await;
    delete(&app, &token, &id).await;

    for target in [MISSING_ID, "abc", id.as_str()] {
        assert_not_found(&delete(&app, &token, target).await);
    }
}

#[tokio::test]
async fn delete_de_registro_de_outro_usuario_da_404_e_o_registro_continua() {
    let app = TestApp::new().await;
    let token_a = app.register_and_login().await;
    let token_b = app.register_and_login_as(maria()).await;
    let id = create(&app, &token_a, valid_body()).await;

    assert_not_found(&delete(&app, &token_b, &id).await);
    assert_eq!(ids(&list(&app, &token_a).await), [id]);
}

// Diferença para o TS ("Diferenças para o front"): o Fastify recusava com 400 um DELETE com Content-Type JSON e
// sem corpo; o axum só lê o corpo quando a rota pede, e esta não pede.
#[tokio::test]
async fn delete_com_content_type_json_e_sem_corpo_ignora_o_corpo_e_apaga() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let id = create(&app, &token, valid_body()).await;

    let response = app
        .delete(&format!("/transactions/{id}"))
        .bearer(&token)
        .header("content-type", "application/json")
        .send()
        .await;

    assert_eq!(response.status, StatusCode::NO_CONTENT);
    assert!(ids(&list(&app, &token).await).is_empty());
}

#[tokio::test]
async fn delete_sem_token_da_401_mesmo_com_id_que_nao_e_uuid() {
    let app = TestApp::new().await;

    for target in [MISSING_ID, "abc"] {
        assert_unauthorized(&app.delete(&format!("/transactions/{target}")).send().await);
    }
}

#[tokio::test]
async fn delete_com_token_revogado_da_401_e_o_registro_continua() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let id = create(&app, &token, valid_body()).await;
    app.post("/auth/logout").bearer(&token).send().await;

    assert_unauthorized(&delete(&app, &token, &id).await);

    let new_token = app.login(DEFAULT_EMAIL, DEFAULT_PASSWORD).await;
    assert_eq!(ids(&list(&app, &new_token).await), [id]);
}

// ---- PATCH /transactions/:id ----

#[tokio::test]
async fn patch_de_um_campo_muda_so_ele_e_devolve_o_registro_inteiro() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for (field, value) in [
        ("description", json!("Almoço (corrigido)")),
        ("type", json!("income")),
        ("amount", json!(2500)),
        ("date", json!("2026-09-30")),
    ] {
        let id = create(&app, &token, valid_body()).await;

        let response = patch(&app, &token, &id, json!({ field: value })).await;

        assert_eq!(response.status, StatusCode::OK, "{field}");
        let body = response.json();
        let mut expected = valid_body();
        expected[field] = value;
        expected["id"] = json!(id);
        expected["createdAt"] = body["createdAt"].clone();
        expected["updatedAt"] = body["updatedAt"].clone();
        assert_eq!(body, expected, "{field}");
    }
}

#[tokio::test]
async fn patch_dos_quatro_campos_juntos_altera_todos() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let id = create(&app, &token, valid_body()).await;
    let changes =
        json!({ "type": "income", "amount": 5000, "description": "Salário", "date": "2026-10-01" });

    let response = patch(&app, &token, &id, changes.clone()).await;

    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    for (key, value) in changes.as_object().unwrap() {
        assert_eq!(&body[key], value, "{key}");
    }
    assert_eq!(body["id"], json!(id));
}

#[tokio::test]
async fn o_get_e_os_totais_refletem_a_mudanca_de_amount_e_de_type() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let id = create(&app, &token, valid_body()).await;

    patch(&app, &token, &id, json!({ "amount": 2500 })).await;
    assert_eq!(
        list(&app, &token).await["summary"],
        json!({ "income": 0, "expense": 2500, "balance": -2500 })
    );

    patch(&app, &token, &id, json!({ "type": "income" })).await;
    let list = list(&app, &token).await;
    assert_eq!(
        list["summary"],
        json!({ "income": 2500, "expense": 0, "balance": 2500 })
    );
    let item = &list["transactions"][0];
    assert_eq!(
        (&item["id"], &item["type"], &item["amount"]),
        (&json!(id), &json!("income"), &json!(2500))
    );
}

#[tokio::test]
async fn editar_nao_faz_o_registro_subir_na_lista() {
    // Mesma date; o mais novo vem antes na lista.
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let older = create(&app, &token, valid_body()).await;
    let newer = create(&app, &token, valid_body()).await;
    let before = ids(&list(&app, &token).await);
    assert_eq!(before, [newer, older.clone()]);

    patch(
        &app,
        &token,
        &older,
        json!({ "description": "primeiro (corrigido)" }),
    )
    .await;

    assert_eq!(ids(&list(&app, &token).await), before);
}

#[tokio::test]
async fn updated_at_passa_a_ser_posterior_e_o_created_at_nao_muda_mesmo_com_os_mesmos_valores() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    // Um valor novo e o próprio valor atual: os dois gravam e atualizam o updatedAt.
    for description in ["Almoço (corrigido)", "Almoço"] {
        let id = create_backdated(&app, &token).await;

        let response = patch(&app, &token, &id, json!({ "description": description })).await;

        assert_eq!(response.status, StatusCode::OK);
        let body = response.json();
        assert_eq!(body["description"], description);
        assert_eq!(body["createdAt"], PAST);
        assert_after_past(&body["updatedAt"]);
        let list = list(&app, &token).await;
        let row = list["transactions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == json!(id))
            .unwrap()
            .clone();
        assert_eq!(
            (&row["createdAt"], &row["updatedAt"]),
            (&json!(PAST), &body["updatedAt"])
        );
    }
}

#[tokio::test]
async fn patch_de_id_inexistente_ou_que_nao_e_uuid_da_404() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;

    for target in [MISSING_ID, "abc"] {
        assert_not_found(&patch(&app, &token, target, json!({ "amount": 2500 })).await);
    }
}

#[tokio::test]
async fn patch_de_registro_de_outro_usuario_da_404_e_nada_muda_nem_o_updated_at() {
    let app = TestApp::new().await;
    let token_a = app.register_and_login().await;
    let token_b = app.register_and_login_as(maria()).await;
    let id = create_backdated(&app, &token_a).await;
    let before = list(&app, &token_a).await;

    let response = patch(
        &app,
        &token_b,
        &id,
        json!({ "amount": 999_999, "description": "invadido" }),
    )
    .await;

    assert_not_found(&response);
    assert_eq!(list(&app, &token_a).await, before);
    let item = &before["transactions"][0];
    assert_eq!(
        (&item["createdAt"], &item["updatedAt"]),
        (&json!(PAST), &json!(PAST))
    );
}

#[tokio::test]
async fn patch_sem_token_da_401_e_com_token_revogado_da_401_sem_alterar_o_registro() {
    let app = TestApp::new().await;
    let token = app.register_and_login().await;
    let id = create(&app, &token, valid_body()).await;
    app.post("/auth/logout").bearer(&token).send().await;

    assert_unauthorized(
        &app.patch(&format!("/transactions/{MISSING_ID}"))
            .json(&json!({ "amount": 2500 }))
            .send()
            .await,
    );
    assert_unauthorized(&patch(&app, &token, &id, json!({ "amount": 2500 })).await);

    let new_token = app.login(DEFAULT_EMAIL, DEFAULT_PASSWORD).await;
    assert_eq!(list(&app, &new_token).await["summary"]["expense"], 1990);
}

// ---- PATCH /transactions/:id: validação do corpo ----

// Um registro recuado no banco e a lista de antes: um PATCH recusado não pode alterar nada (nem o updatedAt).
async fn arrange(app: &TestApp) -> (String, String, Value) {
    let token = app.register_and_login().await;
    let id = create_backdated(app, &token).await;
    let before = list(app, &token).await;
    (token, id, before)
}

#[tokio::test]
async fn patch_invalido_da_400_com_a_mensagem_certa_e_nao_altera_nada() {
    let app = TestApp::new().await;
    let (token, id, before) = arrange(&app).await;
    let cases = [
        ("corpo vazio", json!({}), NO_FIELDS),
        (
            "só campos desconhecidos",
            json!({ "foo": 1, "bar": "x" }),
            NO_FIELDS,
        ),
        // Tipo JSON errado, inclusive null (que não apaga nada).
        ("amount null", json!({ "amount": null }), REQUIRED),
        ("amount string", json!({ "amount": "1990" }), REQUIRED),
        ("type número", json!({ "type": 123 }), REQUIRED),
        ("type null", json!({ "type": null }), REQUIRED),
        ("description null", json!({ "description": null }), REQUIRED),
        ("date número", json!({ "date": 20260929 }), REQUIRED),
        // Tipo certo, valor fora da regra: a mensagem do campo, a mesma do POST.
        ("type inválido", json!({ "type": "foo" }), TYPE_ERROR),
        ("amount decimal", json!({ "amount": 19.9 }), AMOUNT_ERROR),
        ("amount zero", json!({ "amount": 0 }), AMOUNT_ERROR),
        (
            "amount acima do teto",
            json!({ "amount": 100_000_000_001_i64 }),
            AMOUNT_ERROR,
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
            "date inexistente",
            json!({ "date": "2026-02-30" }),
            DATE_ERROR,
        ),
        (
            "date em outro formato",
            json!({ "date": "29/09/2026" }),
            DATE_ERROR,
        ),
    ];

    for (case, body, message) in cases {
        let response = patch(&app, &token, &id, body).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{case}");
        assert_eq!(response.json(), json!({ "error": message }), "{case}");
    }
    assert_eq!(list(&app, &token).await, before);
}

#[tokio::test]
async fn patch_com_corpo_raiz_invalido_da_400_ou_415_e_nao_altera_nada() {
    let app = TestApp::new().await;
    let (token, id, before) = arrange(&app).await;
    let json_type = Some("application/json");
    // Os dois 415 são diferenças para o TS ("Diferenças para o front"), que dava 400.
    let cases = [
        (
            "sem corpo",
            None,
            "",
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            UNSUPPORTED,
        ),
        (
            "application/json vazio",
            json_type,
            "",
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "JSON malformado",
            json_type,
            "{ruim",
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "null",
            json_type,
            "null",
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "array",
            json_type,
            "[]",
            StatusCode::BAD_REQUEST,
            INVALID_BODY,
        ),
        (
            "text/plain",
            Some("text/plain"),
            "oi",
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            UNSUPPORTED,
        ),
    ];

    for (case, content_type, raw, status, message) in cases {
        let mut request = app
            .patch(&format!("/transactions/{id}"))
            .bearer(&token)
            .raw_body(raw);
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        let response = request.send().await;
        assert_eq!(response.status, status, "{case}");
        assert_eq!(response.json(), json!({ "error": message }), "{case}");
    }
    assert_eq!(list(&app, &token).await, before);
}

#[tokio::test]
async fn patch_sem_token_e_com_corpo_malformado_da_401() {
    let app = TestApp::new().await;

    let response = app
        .patch(&format!("/transactions/{MISSING_ID}"))
        .header("content-type", "application/json")
        .raw_body("{ruim")
        .send()
        .await;

    assert_unauthorized(&response);
}

#[tokio::test]
async fn patch_salva_a_descricao_sem_os_espacos_das_pontas() {
    let app = TestApp::new().await;
    let (token, id, _) = arrange(&app).await;

    let response = patch(
        &app,
        &token,
        &id,
        json!({ "description": "  Almoço novo  " }),
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["description"], "Almoço novo");
    assert_eq!(
        list(&app, &token).await["transactions"][0]["description"],
        "Almoço novo"
    );
}

#[tokio::test]
async fn patch_ignora_id_user_id_created_at_e_updated_at_no_corpo() {
    // O A tem o registro; o corpo tenta trocar o dono para o B, o id e as duas datas.
    let app = TestApp::new().await;
    let a = app.register(common::default_user()).await["id"].clone();
    let token_a = app.login(DEFAULT_EMAIL, DEFAULT_PASSWORD).await;
    let b = app.register(maria()).await["id"].clone();
    let id = create_backdated(&app, &token_a).await;

    let response = patch(
        &app,
        &token_a,
        &id,
        json!({
            "amount": 2500,
            "id": MISSING_ID,
            "userId": b,
            "createdAt": "2000-01-01T00:00:00.000Z",
            "updatedAt": "2000-01-01T00:00:00.000Z",
        }),
    )
    .await;

    // Só o amount mudou; o id e o createdAt são os de antes, e o updatedAt veio do banco (now()).
    assert_eq!(response.status, StatusCode::OK);
    let body = response.json();
    assert_eq!(
        (&body["id"], &body["amount"], &body["createdAt"]),
        (&json!(id), &json!(2500), &json!(PAST))
    );
    assert_after_past(&body["updatedAt"]);
    let owner: String =
        sqlx::query_scalar("SELECT user_id::text FROM transactions WHERE id::text = $1")
            .bind(&id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(json!(owner), a);
    let forged: i64 = sqlx::query_scalar("SELECT count(*) FROM transactions WHERE id::text = $1")
        .bind(MISSING_ID)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(forged, 0);
}

#[tokio::test]
async fn ordem_dos_erros_o_id_vem_antes_do_corpo_e_o_corpo_antes_do_dono() {
    let app = TestApp::new().await;
    let token_a = app.register_and_login().await;
    let token_b = app.register_and_login_as(maria()).await;
    let id = create(&app, &token_a, valid_body()).await;
    let invalid = json!({ "amount": 19.9 });

    // :id que não é UUID com corpo inválido → 404.
    assert_not_found(&patch(&app, &token_b, "abc", invalid.clone()).await);
    // id válido de outro usuário com corpo inválido → 400.
    let response = patch(&app, &token_b, &id, invalid).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.json(), json!({ "error": AMOUNT_ERROR }));
}
