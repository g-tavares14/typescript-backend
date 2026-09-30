import { afterEach, beforeEach, describe, expect, test } from "vitest";
import { bearer, closeTestApp, createTestApp, defaultUser, resetDatabase } from "./helpers.ts";

const RATE_LIMIT_ERROR = { error: "Muitas tentativas. Tente novamente mais tarde." };

// Cada teste usa um app NOVO com o rate limit ligado e os limites reais (5 logins e 3 cadastros por minuto):
// os contadores ficam na memória do app, então um app por teste evita que um teste consuma o limite do outro.
// Cada app abre o próprio pool de conexões, por isso o afterEach fecha todos.
let current: ReturnType<typeof createTestApp>;

beforeEach(async () => {
  current = createTestApp({ rateLimit: true });
  await resetDatabase(current.db);
});

afterEach(async () => {
  await closeTestApp(current.app, current.db);
});

function postLogin(payload: object, remoteAddress?: string) {
  return current.app.inject({ method: "POST", url: "/auth/login", payload, remoteAddress });
}

function postRegister(payload: object, remoteAddress?: string) {
  return current.app.inject({ method: "POST", url: "/auth/register", payload, remoteAddress });
}

function newUser(n: number) {
  return { username: `usuario_${n}`, email: `usuario_${n}@email.com`, password: "senha12345" };
}

const wrongLogin = { email: defaultUser.email, password: "senha-errada" };

describe("rate limit em POST /auth/login (5 por minuto por IP)", () => {
  test("as 5 primeiras tentativas passam (401 por senha errada) e a 6ª responde 429", async () => {
    // Act + Assert: tentativas com senha errada também contam.
    for (let i = 1; i <= 5; i++) {
      expect((await postLogin(wrongLogin)).statusCode).toBe(401);
    }
    const blocked = await postLogin(wrongLogin);

    expect(blocked.statusCode).toBe(429);
    expect(blocked.json()).toEqual(RATE_LIMIT_ERROR);
    const retryAfter = Number(blocked.headers["retry-after"]);
    expect(Number.isInteger(retryAfter)).toBe(true);
    expect(retryAfter).toBeGreaterThan(0);
    expect(retryAfter).toBeLessThanOrEqual(60);
  });

  test("o limite é checado antes de validar o corpo: corpo inválido também recebe 429", async () => {
    // Arrange
    for (let i = 1; i <= 5; i++) {
      await postLogin(wrongLogin);
    }

    // Act
    const response = await postLogin({});

    // Assert: sem o limite, este corpo receberia 400.
    expect(response.statusCode).toBe(429);
    expect(response.json()).toEqual(RATE_LIMIT_ERROR);
  });

  test("o limite é por IP: outro IP ainda consegue entrar", async () => {
    // Arrange
    await postRegister(defaultUser);
    for (let i = 1; i <= 5; i++) {
      await postLogin(wrongLogin);
    }
    expect((await postLogin(wrongLogin)).statusCode).toBe(429);

    // Act
    const otherIp = await postLogin(
      { email: defaultUser.email, password: defaultUser.password },
      "10.0.0.2",
    );

    // Assert
    expect(otherIp.statusCode).toBe(200);
  });
});

describe("rate limit em POST /auth/register (3 por minuto por IP)", () => {
  test("os 3 primeiros cadastros passam e o 4º responde 429", async () => {
    for (let i = 1; i <= 3; i++) {
      expect((await postRegister(newUser(i))).statusCode).toBe(201);
    }
    const blocked = await postRegister(newUser(4));

    expect(blocked.statusCode).toBe(429);
    expect(blocked.json()).toEqual(RATE_LIMIT_ERROR);
    expect(Number(blocked.headers["retry-after"])).toBeGreaterThan(0);
  });

  test("o limite é checado antes de validar o corpo: corpo inválido também recebe 429", async () => {
    // Arrange
    for (let i = 1; i <= 3; i++) {
      await postRegister(newUser(i));
    }

    // Act
    const response = await postRegister({});

    // Assert
    expect(response.statusCode).toBe(429);
    expect(response.json()).toEqual(RATE_LIMIT_ERROR);
  });
});

// Cadastra e faz login (esses dois usam os limites das próprias rotas, sem relação com os de /users/me).
async function tokenForDefaultUser() {
  await postRegister(defaultUser);
  const login = await postLogin({ email: defaultUser.email, password: defaultUser.password });
  return login.json<{ token: string }>().token;
}

function deleteMe(token: string | undefined, password = "senha-errada") {
  return current.app.inject({
    method: "DELETE",
    url: "/users/me",
    headers: token === undefined ? {} : { authorization: bearer(token) },
    payload: { password },
  });
}

function patchMe(token: string | undefined, username = "novo_nome") {
  return current.app.inject({
    method: "PATCH",
    url: "/users/me",
    headers: token === undefined ? {} : { authorization: bearer(token) },
    payload: { username },
  });
}

describe("rate limit em DELETE /users/me (5 por minuto por IP)", () => {
  test("as 5 primeiras tentativas com senha errada dão 403 e a 6ª responde 429", async () => {
    const token = await tokenForDefaultUser();

    for (let i = 1; i <= 5; i++) {
      expect((await deleteMe(token)).statusCode).toBe(403);
    }
    const blocked = await deleteMe(token);

    expect(blocked.statusCode).toBe(429);
    expect(blocked.json()).toEqual(RATE_LIMIT_ERROR);
    const retryAfter = Number(blocked.headers["retry-after"]);
    expect(retryAfter).toBeGreaterThan(0);
    expect(retryAfter).toBeLessThanOrEqual(60);
  });

  test("com o limite estourado, nem a senha certa apaga a conta", async () => {
    const token = await tokenForDefaultUser();
    for (let i = 1; i <= 5; i++) {
      await deleteMe(token);
    }

    const blocked = await deleteMe(token, defaultUser.password);

    expect(blocked.statusCode).toBe(429);
    // O 429 sai antes do handler: a conta continua existindo (o GET /users/me não tem limite).
    const me = await current.app.inject({
      method: "GET",
      url: "/users/me",
      headers: { authorization: bearer(token) },
    });
    expect(me.statusCode).toBe(200);
  });

  test("sem token responde 401 e não consome o limite (o requireAuth roda antes)", async () => {
    const token = await tokenForDefaultUser();

    for (let i = 1; i <= 6; i++) {
      const response = await deleteMe(undefined);
      expect(response.statusCode).toBe(401);
    }

    // Se as 6 requisições sem token tivessem contado, esta (a 7ª) já seria 429.
    for (let i = 1; i <= 5; i++) {
      expect((await deleteMe(token)).statusCode).toBe(403);
    }
    expect((await deleteMe(token)).statusCode).toBe(429);
  });
});

describe("rate limit em PATCH /users/me (10 por minuto por IP)", () => {
  test("os 10 primeiros passam (200) e o 11º responde 429", async () => {
    const token = await tokenForDefaultUser();

    for (let i = 1; i <= 10; i++) {
      expect((await patchMe(token, `nome_${i}`)).statusCode).toBe(200);
    }
    const blocked = await patchMe(token, "nome_11");

    expect(blocked.statusCode).toBe(429);
    expect(blocked.json()).toEqual(RATE_LIMIT_ERROR);
    expect(Number(blocked.headers["retry-after"])).toBeGreaterThan(0);
  });

  test("sem token responde 401 e não consome o limite", async () => {
    const token = await tokenForDefaultUser();

    for (let i = 1; i <= 11; i++) {
      expect((await patchMe(undefined)).statusCode).toBe(401);
    }

    for (let i = 1; i <= 10; i++) {
      expect((await patchMe(token, `nome_${i}`)).statusCode).toBe(200);
    }
    expect((await patchMe(token)).statusCode).toBe(429);
  });

  test("PATCH e DELETE têm contadores separados", async () => {
    const token = await tokenForDefaultUser();
    // Esgota o PATCH (10). Com um contador só, o DELETE seguinte já daria 429.
    for (let i = 1; i <= 10; i++) {
      expect((await patchMe(token)).statusCode).toBe(200);
    }
    expect((await patchMe(token)).statusCode).toBe(429);

    expect((await deleteMe(token)).statusCode).toBe(403);
  });
});

describe("rotas sem limite", () => {
  test("GET /users/me não é limitado: 10 chamadas seguidas com token válido dão 200", async () => {
    // Arrange
    await postRegister(defaultUser);
    const login = await postLogin({ email: defaultUser.email, password: defaultUser.password });
    const { token } = login.json<{ token: string }>();

    // Act + Assert
    for (let i = 1; i <= 10; i++) {
      const response = await current.app.inject({
        method: "GET",
        url: "/users/me",
        headers: { authorization: bearer(token) },
      });
      expect(response.statusCode).toBe(200);
    }
  });
});
