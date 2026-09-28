import { eq } from "drizzle-orm";
import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { users } from "../src/db/schema.ts";
import { closeTestApp, createTestApp, defaultUser, registerUser, resetDatabase } from "./helpers.ts";

const { app, db } = createTestApp();

const USERNAME_FORMAT_ERROR = "O username só pode ter letras sem acento, números e _";

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

function postRegister(payload: object) {
  return app.inject({ method: "POST", url: "/auth/register", payload });
}

describe("POST /auth/register", () => {
  test("cria o usuário e responde 201", async () => {
    // Act
    const response = await postRegister(defaultUser);

    // Assert
    expect(response.statusCode).toBe(201);
    expect(response.json()).toEqual({
      id: expect.any(String),
      username: "joao",
    });
  });

  test("responde 409 quando o email já está cadastrado", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await postRegister({ ...defaultUser, username: "outro_nome" });

    // Assert
    expect(response.statusCode).toBe(409);
    expect(response.json()).toEqual({ error: "Email ou username já cadastrado" });
  });

  test("responde 400 quando a senha tem menos de 8 caracteres", async () => {
    // Act
    const response = await postRegister({ ...defaultUser, password: "123" });

    // Assert
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error: "A senha deve ter no mínimo 8 caracteres" });
  });

  test("responde 409 quando o username já está cadastrado", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await postRegister({ ...defaultUser, email: "outro@email.com" });

    // Assert
    expect(response.statusCode).toBe(409);
    expect(response.json()).toEqual({ error: "Email ou username já cadastrado" });
  });

  test("normaliza o email: mesmo email com maiúsculas e espaços é duplicado", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await postRegister({ ...defaultUser, username: "outro_nome", email: "  JOAO@Email.com " });

    // Assert
    expect(response.statusCode).toBe(409);
  });

  test("normaliza o username: salva e devolve em minúsculas", async () => {
    // Act
    const response = await postRegister({ ...defaultUser, username: "Joao" });

    // Assert
    expect(response.statusCode).toBe(201);
    expect(response.json()).toEqual({ id: expect.any(String), username: "joao" });
  });

  test("responde 409 quando só as maiúsculas do username mudam (Joao depois de joao)", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await postRegister({ ...defaultUser, username: "Joao", email: "outro@email.com" });

    // Assert
    expect(response.statusCode).toBe(409);
    expect(response.json()).toEqual({ error: "Email ou username já cadastrado" });
  });

  test.each([
    ["corpo vazio", {}, "Campo obrigatório ausente ou inválido"],
    ["email inválido", { ...defaultUser, email: "nao-e-email" }, "Email inválido"],
    ["username curto", { ...defaultUser, username: "jo" }, "O username deve ter entre 3 e 50 caracteres"],
    ["username com acento", { ...defaultUser, username: "joão" }, USERNAME_FORMAT_ERROR],
    ["username com espaço", { ...defaultUser, username: "jo ao" }, USERNAME_FORMAT_ERROR],
    // "о" abaixo é a letra cirílica U+043E, visualmente igual ao "o" latino.
    [
      "username com letra cirílica parecida com latina",
      { ...defaultUser, username: "j\u043eao" },
      USERNAME_FORMAT_ERROR,
    ],
    [
      "username com caractere invisível (zero-width space)",
      { ...defaultUser, username: "joao\u200b" },
      USERNAME_FORMAT_ERROR,
    ],
    ["username com hífen", { ...defaultUser, username: "joao-silva" }, USERNAME_FORMAT_ERROR],
    ["campo com tipo errado", { ...defaultUser, password: 12345678 }, "Campo obrigatório ausente ou inválido"],
  ])("responde 400 com %s", async (_caso, payload, error) => {
    const response = await postRegister(payload);

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error });
  });

  test("salva só o hash argon2id da senha, nunca a senha em texto puro", async () => {
    // Act
    const { id } = await registerUser(app);

    // Assert
    const [user] = await db.select().from(users).where(eq(users.id, id));
    expect(user?.passwordHash).toMatch(/^\$argon2id\$/);
    expect(user?.passwordHash).not.toContain("senha123");
  });

  test("ignora a role enviada na requisição: todo cadastro nasce como user", async () => {
    // Act
    const { id } = await registerUser(app, { ...defaultUser, role: "admin" });

    // Assert
    const [user] = await db.select({ role: users.role }).from(users).where(eq(users.id, id));
    expect(user?.role).toBe("user");
  });
});
