import type { FastifyInstance } from "fastify";

// Modo paridade: com API_URL definida (npm run test:parity), os testes falam por HTTP com outro servidor (o Rust)
// em vez de chamar o Fastify com app.inject(). O global-setup sobe esse servidor apontado para o banco de testes.
export const API_URL = process.env.API_URL;
export const isParity = API_URL !== undefined;

// Só as opções do inject que os testes usam.
type InjectOptions = {
  method: string;
  url: string;
  headers?: Record<string, string>;
  payload?: string | object;
};

// A parte da resposta do inject que os testes leem.
type InjectResponse = {
  statusCode: number;
  headers: Record<string, string>;
  body: string;
  json<T = unknown>(): T;
};

async function inject({ method, url, headers = {}, payload }: InjectOptions): Promise<InjectResponse> {
  const requestHeaders = new Headers(headers);
  // Blob sem tipo, e não string: com string o fetch adiciona `Content-Type: text/plain` sozinho, e o inject não.
  let body: Blob | undefined;
  if (typeof payload === "string") {
    body = new Blob([payload]);
  } else if (payload !== undefined) {
    // Objeto: como o inject, serializa e marca como JSON (se o teste não mandou outro Content-Type).
    body = new Blob([JSON.stringify(payload)]);
    if (!requestHeaders.has("content-type")) {
      requestHeaders.set("content-type", "application/json");
    }
  }

  const response = await fetch(new URL(url, API_URL), { method, headers: requestHeaders, body });
  const text = await response.text();
  return {
    statusCode: response.status,
    headers: Object.fromEntries(response.headers),
    body: text,
    json: () => JSON.parse(text),
  };
}

// Um "app" que só sabe inject() e close(). O cast mantém o tipo do Fastify para os testes compilarem sem mudança;
// qualquer outro método (addHook, get, setChildLoggerFactory...) falha alto: teste que usa internos do Fastify é
// só-TS e precisa estar marcado com `describe.skipIf(isParity)` / `test.skipIf(isParity)`.
export function createHttpApp(): FastifyInstance {
  const app = { inject, close: async () => {}, ready: async () => {} };
  return new Proxy(app, {
    get(target, property) {
      if (property in target) {
        return target[property as keyof typeof target];
      }
      throw new Error(`Modo paridade: app.${String(property)} não existe por HTTP (marque o teste como só-TS)`);
    },
  }) as unknown as FastifyInstance;
}
