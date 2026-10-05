import { DrizzleQueryError } from "drizzle-orm";
import type { FastifyError, FastifyReply, FastifyRequest } from "fastify";
import { afterAll, afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { sendError } from "../src/lib/errors.ts";
import { isParity } from "./parity.ts";
import { bearer, closeTestApp, createTestApp, expectUnauthorized, loginUser, registerUser, resetDatabase } from "./helpers.ts";

const { app, db } = createTestApp();

// O logger do app de teste é desligado, então a fábrica de child loggers troca warn/error por um registro
// em memória: dá para conferir QUE e O QUE foi logado sem depender do formato do pino.
type LogCall = { level: "warn" | "error"; args: unknown[] };
function captureLogs(target: typeof app, calls: LogCall[]) {
  target.setChildLoggerFactory((logger, bindings, opts) => {
    const child = logger.child(bindings, opts);
    child.warn = (...args: unknown[]) => calls.push({ level: "warn", args });
    child.error = (...args: unknown[]) => calls.push({ level: "error", args });
    return child;
  });
}
const logged: LogCall[] = [];

// Rotas de teste que lançam erros 4xx no formato do Fastify, para provar o mapeamento por error.code.
// Registradas antes do primeiro inject (o app só fecha a lista de rotas no primeiro uso).
// Só no modo TS: no modo paridade o app é um cliente HTTP e não tem logger nem rotas de teste.
if (!isParity) {
  captureLogs(app, logged);
  app.get("/teste-fst-sem-mapeamento", async () => {
    throw Object.assign(new Error("English message from the framework"), { code: "FST_ERR_QUALQUER_COISA", statusCode: 422 });
  });
  app.get("/teste-4xx-sem-fst", async () => {
    throw Object.assign(new Error("Mensagem própria da rota"), { statusCode: 409 });
  });
}

beforeEach(async () => {
  logged.length = 0;
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

const INVALID_BODY = { error: "Corpo da requisição inválido: envie um objeto JSON" };
const JSON_HEADERS = { "content-type": "application/json" };
const UNSUPPORTED = { error: "Tipo de conteúdo não suportado (use application/json)" };

// Diferença documentada (SPEC-migracao-rust.md, "Diferenças para o front"): sem Content-Type ou com text/plain,
// o Fastify lê o corpo (400) e o axum recusa o tipo (415).
const RUST_UNSUPPORTED_CASES = new Set(["sem corpo e sem content-type", "text/plain"]);
// Diferença documentada: o Fastify recusa `__proto__` (prototype pollution); no Rust é uma chave qualquer, e o corpo
// segue para a validação dos campos (sem nenhum campo obrigatório: "Campo obrigatório ausente ou inválido").
const RUST_FIELD_VALIDATION_CASES = new Set(["JSON com __proto__"]);

// Corpos que não são um objeto JSON utilizável. Cada caso vira 400 com a mesma mensagem, em qualquer rota com corpo.
const invalidBodies: Array<[string, { headers?: Record<string, string>; payload?: string }]> = [
  ["sem corpo e sem content-type", {}],
  ["application/json vazio", { headers: JSON_HEADERS, payload: "" }],
  ["JSON malformado", { headers: JSON_HEADERS, payload: "{ruim" }],
  ["JSON com __proto__", { headers: JSON_HEADERS, payload: '{"__proto__":{"admin":true}}' }],
  ["null", { headers: JSON_HEADERS, payload: "null" }],
  ["array", { headers: JSON_HEADERS, payload: "[]" }],
  ["string JSON", { headers: JSON_HEADERS, payload: '"x"' }],
  ["text/plain", { headers: { "content-type": "text/plain" }, payload: "oi" }],
];

describe("corpo inválido: 400 em português nas 3 rotas com corpo", () => {
  describe.each([
    ["POST /auth/register", "/auth/register"],
    ["POST /auth/login", "/auth/login"],
    ["POST /transactions (autenticado)", "/transactions"],
  ])("%s", (_nome, url) => {
    test.each(invalidBodies)("%s", async (caso, request) => {
      // Arrange: só a rota protegida precisa de token.
      const headers = { ...request.headers } as Record<string, string>;
      if (url === "/transactions") {
        await registerUser(app);
        headers.authorization = bearer(await loginUser(app));
      }

      // Act
      const response = await app.inject({ method: "POST", url, headers, payload: request.payload });

      // Assert
      if (isParity && RUST_UNSUPPORTED_CASES.has(caso)) {
        expect(response.statusCode).toBe(415);
        expect(response.json()).toEqual(UNSUPPORTED);
        return;
      }
      if (isParity && RUST_FIELD_VALIDATION_CASES.has(caso)) {
        expect(response.statusCode).toBe(400);
        expect(response.json()).toEqual({ error: "Campo obrigatório ausente ou inválido" });
        return;
      }
      expect(response.statusCode).toBe(400);
      expect(response.json()).toEqual(INVALID_BODY);
    });
  });

  // Só-TS: o fetch não deixa mandar um Content-Length diferente do corpo. Equivalente em Rust: rust/tests/errors.rs.
  test.skipIf(isParity)("Content-Length que não confere com o corpo também é corpo inválido", async () => {
    const response = await app.inject({
      method: "POST",
      url: "/auth/login",
      headers: { ...JSON_HEADERS, "content-length": "5" },
      payload: '{"email":"a@b.com","password":"x"}',
    });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(INVALID_BODY);
  });

  test("com um objeto JSON, as mensagens dos campos continuam as de cada campo (não viram a do corpo)", async () => {
    const register = await app.inject({ method: "POST", url: "/auth/register", payload: {} });
    const login = await app.inject({ method: "POST", url: "/auth/login", payload: {} });

    expect(register.json()).toEqual({ error: "Campo obrigatório ausente ou inválido" });
    expect(login.json()).toEqual({ error: "Campo obrigatório ausente ou inválido" });
  });
});

describe("erros do Fastify mapeados por error.code", () => {
  test("tipo de conteúdo sem parser (application/xml) → 415", async () => {
    const response = await app.inject({
      method: "POST",
      url: "/auth/login",
      headers: { "content-type": "application/xml" },
      payload: "<a/>",
    });

    expect(response.statusCode).toBe(415);
    expect(response.json()).toEqual({ error: "Tipo de conteúdo não suportado (use application/json)" });
  });

  test("corpo maior que 1 MiB → 413", async () => {
    const response = await app.inject({
      method: "POST",
      url: "/auth/login",
      headers: JSON_HEADERS,
      payload: JSON.stringify({ email: "a".repeat(1024 * 1024) }),
    });

    expect(response.statusCode).toBe(413);
    expect(response.json()).toEqual({ error: "Corpo da requisição muito grande" });
  });

  test("rota inexistente → 404 sem repetir a URL", async () => {
    const response = await app.inject({ method: "GET", url: "/nada?segredo=1" });

    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual({ error: "Rota não encontrada" });
    expect(response.body).not.toContain("nada");
  });

  test("método inexistente numa rota que existe → 404 (405 no Rust)", async () => {
    const response = await app.inject({ method: "DELETE", url: "/transactions" });

    // Diferença documentada: o axum responde 405 para método errado numa rota que existe.
    if (isParity) {
      expect(response.statusCode).toBe(405);
      expect(response.json()).toEqual({ error: "Método não permitido" });
      return;
    }
    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual({ error: "Rota não encontrada" });
  });

  test("URL malformada → 400 sem repetir a URL (404 no Rust)", async () => {
    const response = await app.inject({ method: "GET", url: "/%E0%A4%A" });

    expect(response.body).not.toContain("E0");
    // Diferença documentada: o axum não decodifica o caminho para escolher a rota, então não acha nenhuma.
    if (isParity) {
      expect(response.statusCode).toBe(404);
      expect(response.json()).toEqual({ error: "Rota não encontrada" });
      return;
    }
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error: "URL inválida" });
  });

  test.skipIf(isParity)("outro 4xx do Fastify (code FST_*) sem mapeamento → 'Requisição inválida', mantendo o status", async () => {
    const response = await app.inject({ method: "GET", url: "/teste-fst-sem-mapeamento" });

    expect(response.statusCode).toBe(422);
    expect(response.json()).toEqual({ error: "Requisição inválida" });
  });

  test.skipIf(isParity)("4xx FST_* sem mapeamento loga só o código, nunca a mensagem original nem a URL", async () => {
    await app.inject({ method: "GET", url: "/teste-fst-sem-mapeamento?segredo=1" });

    expect(logged).toHaveLength(1);
    expect(logged[0]?.level).toBe("warn");
    expect(logged[0]?.args).toEqual([{ code: "FST_ERR_QUALQUER_COISA" }, "Erro 4xx sem mensagem mapeada"]);
  });

  test.skipIf(isParity)("4xx mapeados e 4xx sem FST_* não são logados", async () => {
    await app.inject({ method: "POST", url: "/auth/login", headers: { "content-type": "application/xml" }, payload: "<a/>" });
    await app.inject({ method: "POST", url: "/auth/login", headers: JSON_HEADERS, payload: "{ruim" });
    await app.inject({ method: "GET", url: "/teste-4xx-sem-fst" });
    await app.inject({ method: "GET", url: "/nada" });

    expect(logged).toEqual([]);
  });

  test.skipIf(isParity)("4xx sem code FST_* continua com a própria mensagem", async () => {
    const response = await app.inject({ method: "GET", url: "/teste-4xx-sem-fst" });

    expect(response.statusCode).toBe(409);
    expect(response.json()).toEqual({ error: "Mensagem própria da rota" });
  });
});

describe("o que não pode mudar", () => {
  test("o 401 continua vindo antes de qualquer erro de corpo nas rotas protegidas", async () => {
    const cases = [
      { headers: JSON_HEADERS, payload: "{ruim" },
      { headers: { "content-type": "application/xml" }, payload: "<a/>" },
      { headers: JSON_HEADERS, payload: JSON.stringify({ description: "a".repeat(1024 * 1024) }) },
      {},
    ];

    for (const request of cases) {
      expectUnauthorized(await app.inject({ method: "POST", url: "/transactions", ...request }));
    }
  });

  // Só-TS: rate limit fica de fora da paridade HTTP (contadores no processo); coberto por testes em Rust (T11).
  describe.skipIf(isParity)("429 do rate limit", () => {
    // App próprio com o rate limit ligado: os contadores ficam na memória do app.
    let limited: ReturnType<typeof createTestApp>;

    beforeEach(() => {
      limited = createTestApp({ rateLimit: true });
    });

    afterEach(async () => {
      await closeTestApp(limited.app, limited.db);
    });

    test("continua com a mensagem própria em português", async () => {
      // Act: 5 tentativas passam (400 por corpo vazio) e a 6ª é bloqueada.
      for (let i = 0; i < 5; i++) {
        await limited.app.inject({ method: "POST", url: "/auth/login", payload: {} });
      }
      const blocked = await limited.app.inject({ method: "POST", url: "/auth/login", payload: {} });

      // Assert
      expect(blocked.statusCode).toBe(429);
      expect(blocked.json()).toEqual({ error: "Muitas tentativas. Tente novamente mais tarde." });
    });
  });
});

// Erros 5xx gerados pelo próprio Fastify antes de escolher a rota (frameworkErrors) seguem a regra de todo 5xx:
// mensagem genérica para o cliente e o erro completo no log. O caso real: uma constraint assíncrona que falha
// (FST_ERR_ASYNC_CONSTRAINT, status 500). App próprio, porque a constraint vale para todas as requisições dele.
// Só-TS: depende de internos do Fastify. Equivalente em Rust: pânico/erro interno em rust/tests/errors.rs.
describe.skipIf(isParity)("5xx gerado pelo framework (frameworkErrors)", () => {
  if (isParity) {
    return; // o vitest ainda executa o corpo de um describe pulado para coletar os testes
  }
  const constrained = createTestApp();
  // Os tipos do find-my-way não descrevem a forma assíncrona (deriveConstraint com callback `done`), daí o cast.
  const failingStrategy = {
    name: "falha",
    storage: () => {
      const handlers: Record<string, unknown> = {};
      return { get: (type: string) => handlers[type] ?? null, set: (type: string, store: unknown) => (handlers[type] = store) };
    },
    // 3 parâmetros = constraint assíncrona; devolve erro em toda requisição.
    deriveConstraint: (_request: unknown, _context: unknown, done: (error: Error) => void) =>
      done(new Error("falha secreta do banco de constraints")),
    mustMatchWhenDerived: false,
  };
  constrained.app.addConstraintStrategy(failingStrategy as unknown as Parameters<typeof app.addConstraintStrategy>[0]);
  constrained.app.get("/qualquer", { constraints: { falha: "x" } }, async () => ({ ok: true }));

  afterAll(async () => {
    await closeTestApp(constrained.app, constrained.db);
  });

  test("responde 500 'Erro interno do servidor', sem vazar a mensagem do framework", async () => {
    const response = await constrained.app.inject({ method: "GET", url: "/qualquer" });

    expect(response.statusCode).toBe(500);
    expect(response.json()).toEqual({ error: "Erro interno do servidor" });
    expect(response.body).not.toMatch(/constraint|falha secreta/i);
  });
});

// O log do 5xx que vem do frameworkErrors não dá para capturar pelo app de teste (o Fastify cria esse logger antes de
// existir qualquer rota, fora da fábrica de child loggers). Por isso a regra de log é conferida direto na função
// que as duas portas (setErrorHandler e frameworkErrors) usam.
// Só-TS: testa a função do TS diretamente. Equivalente em Rust: rust/tests/errors.rs.
describe.skipIf(isParity)("sendError (a função das duas portas de erro)", () => {
  function callSendError(error: Partial<FastifyError>) {
    const log = { error: vi.fn(), warn: vi.fn() };
    const sent: { status?: number; body?: unknown } = {};
    const reply = {
      code(status: number) {
        sent.status = status;
        return this;
      },
      send(body: unknown) {
        sent.body = body;
        return this;
      },
    };
    sendError(error as FastifyError, { log } as unknown as FastifyRequest, reply as unknown as FastifyReply);
    return { log, sent };
  }

  test("5xx (mesmo com code FST_*): 500 genérico e o erro completo no log", () => {
    const error = Object.assign(new Error("detalhe interno"), { code: "FST_ERR_ASYNC_CONSTRAINT", statusCode: 500 });

    const { log, sent } = callSendError(error);

    expect(sent).toEqual({ status: 500, body: { error: "Erro interno do servidor" } });
    expect(log.error).toHaveBeenCalledExactlyOnceWith(error);
    expect(log.warn).not.toHaveBeenCalled();
  });

  test("erro sem statusCode é tratado como 5xx", () => {
    const { log, sent } = callSendError(new Error("qualquer coisa") as FastifyError);

    expect(sent).toEqual({ status: 500, body: { error: "Erro interno do servidor" } });
    expect(log.error).toHaveBeenCalledOnce();
  });

  test("erro do banco (5xx): loga só a consulta e o erro original, nunca os parâmetros", () => {
    const cause = new Error("connection refused");
    const error = new DrizzleQueryError("select ...", ["joao@email.com", "hash-secreto"], cause) as unknown as FastifyError;

    const { log, sent } = callSendError(error);

    expect(sent.body).toEqual({ error: "Erro interno do servidor" });
    expect(log.error).toHaveBeenCalledExactlyOnceWith({ query: "select ...", err: cause }, "Erro no banco de dados");
  });
});
