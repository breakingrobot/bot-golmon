// Génération du client typé à partir de la spécification OpenAPI du bot.
//
// `ui/openapi.json` est exporté par un test Rust (même contenu que GET /api/openapi.json).
// `npm run generate:api` écrit dans src/api/generated/ (non versionné, exclu du lint et du format).
// Les pages n'importent que src/api/ (queries.ts) : passer des types écrits à la main aux
// types générés ne touche que ce dossier.

import { defineConfig } from '@hey-api/openapi-ts'

export default defineConfig({
  input: './openapi.json',
  output: { path: './src/api/generated' },
  plugins: [
    {
      name: '@hey-api/client-fetch',
      // Même origine, cookie de session, en-tête CSRF sur les écritures (voir src/api/hey-api.ts).
      runtimeConfigPath: './src/api/hey-api',
    },
    '@hey-api/typescript',
    '@hey-api/sdk',
    '@tanstack/react-query',
  ],
})
