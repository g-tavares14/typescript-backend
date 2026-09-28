import { readFileSync } from "node:fs";
import { parseEnv } from "node:util";
import { defineConfig } from "vitest/config";

// Carrega o .env.test SOBRESCREVENDO o ambiente, para os testes nunca apontarem
// para o banco de desenvolvimento por causa de uma variável exportada no terminal.
Object.assign(process.env, parseEnv(readFileSync(".env.test", "utf8")));

export default defineConfig({
  test: {
    include: ["test/**/*.test.ts"],
    globalSetup: "./test/global-setup.ts",
    // Todos os testes usam o mesmo banco: rodar os arquivos um de cada vez evita que um apague os dados do outro.
    fileParallelism: false,
  },
});
