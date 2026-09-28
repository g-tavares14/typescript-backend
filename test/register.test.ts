import { eq } from "drizzle-orm";
import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { users } from "../src/db/schema.ts";
import { closeTestApp, createTestApp, registerUser, resetDatabase } from "./helpers.ts";

const { app, db } = createTestApp();

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

describe("POST /auth/register", () => {
  test("cria o usuário e responde 201", async () => {
    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/register",
      payload: { username: "joao", email: "joao@email.com", password: "senha123" },
    });

    // Assert
    expect(response.statusCode).toBe(201);
    expect(response.json()).toEqual({
      id: expect.any(String),
      username: "joao",
    });
  });

  test("responde 409 quando o email já está cadastrado", async () => {
    // Arrange
    await app.inject({
      method: "POST",
      url: "/auth/register",
      payload: { username: "joao", email: "joao@email.com", password: "senha123" },
    });

    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/register",
      payload: { username: "outro_nome", email: "joao@email.com", password: "outrasenha123" },
    });

    // Assert
    expect(response.statusCode).toBe(409);
    expect(response.json()).toEqual({ error: "Email ou username já cadastrado" });
  });

  test("responde 400 quando a senha tem menos de 8 caracteres", async () => {
    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/register",
      payload: { username: "marcos", email: "marcos@email.com", password: "123" },
    });

    // Assert
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error: "A senha deve ter no mínimo 8 caracteres" });
  });

  test("responde 409 quando o username já está cadastrado", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/register",
      payload: { username: "joao", email: "outro@email.com", password: "senha123" },
    });

    // Assert
    expect(response.statusCode).toBe(409);
    expect(response.json()).toEqual({ error: "Email ou username já cadastrado" });
  });

  test("normaliza o email: mesmo email com maiúsculas e espaços é duplicado", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/register",
      payload: { username: "outro_nome", email: "  JOAO@Email.com ", password: "senha123" },
    });

    // Assert
    expect(response.statusCode).toBe(409);
  });

  test.each([
    ["corpo vazio", {}, "Campo obrigatório ausente ou inválido"],
    ["email inválido", { username: "joao", email: "nao-e-email", password: "senha123" }, "Email inválido"],
    [
      "username curto",
      { username: "jo", email: "joao@email.com", password: "senha123" },
      "O username deve ter entre 3 e 50 caracteres",
    ],
    [
      "campo com tipo errado",
      { username: "joao", email: "joao@email.com", password: 12345678 },
      "Campo obrigatório ausente ou inválido",
    ],
  ])("responde 400 com %s", async (_caso, payload, error) => {
    const response = await app.inject({ method: "POST", url: "/auth/register", payload });

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
    const { id } = await registerUser(app, {
      username: "joao",
      email: "joao@email.com",
      password: "senha123",
      role: "admin",
    });

    // Assert
    const [user] = await db.select({ role: users.role }).from(users).where(eq(users.id, id));
    expect(user?.role).toBe("user");
  });
});
