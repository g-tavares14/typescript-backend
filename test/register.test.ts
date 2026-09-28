import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { closeTestApp, createTestApp, resetDatabase } from "./helpers.ts";

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
});
