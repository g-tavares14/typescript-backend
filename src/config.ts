// Lê as variáveis de ambiente na inicialização.
// Se faltar algo essencial, o servidor nem sobe (fail fast).
function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Variável de ambiente ${name} não definida`);
  }
  return value;
}

export const config = {
  databaseUrl: requireEnv("DATABASE_URL"),
  port: Number(process.env.PORT ?? 3000),
};
