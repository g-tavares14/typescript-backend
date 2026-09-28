// Lê as variáveis de ambiente na inicialização.
// Se faltar algo essencial, o servidor nem sobe (fail fast).
function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Variável de ambiente ${name} não definida`);
  }
  return value;
}

function requireJwtSecret(): string {
  const secret = requireEnv("JWT_SECRET");
  // Um segredo curto (ou o valor de exemplo do .env.example) pode ser adivinhado por força bruta.
  if (secret.length < 32 || secret.startsWith("troque-por")) {
    throw new Error("JWT_SECRET deve ser um valor aleatório com pelo menos 32 caracteres");
  }
  return secret;
}

export const config = {
  databaseUrl: requireEnv("DATABASE_URL"),
  jwtSecret: requireJwtSecret(),
  port: Number(process.env.PORT ?? 3000),
};
