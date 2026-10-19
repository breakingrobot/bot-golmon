// Configuration d'exécution du client généré par Hey API (openapi-ts.config.ts → runtimeConfigPath).
// Mêmes règles que client.ts : même origine, cookie de session, `X-Requested-With: fetch`
// sur les écritures (protection CSRF exigée par le serveur).

const fetchWithCsrf: typeof fetch = (input, init) => {
  const request = new Request(input, init)
  if (request.method !== 'GET' && request.method !== 'HEAD') request.headers.set('X-Requested-With', 'fetch')
  return fetch(request)
}

export const createClientConfig = <T extends object>(config?: T) => ({
  ...config,
  baseUrl: '',
  credentials: 'same-origin' as const,
  fetch: fetchWithCsrf,
})
