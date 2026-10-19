import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'
import viteReact from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// Le bot sert `dist/` (index.html pour toutes les routes du SPA) et l'API sous `/api`.
// En développement, `/api`, `/login`, `/logout`, `/auth` et le texte brut des logs
// sont relayés vers le bot local (BOT_URL, port 8890 par défaut).
const bot = process.env.BOT_URL ?? 'http://localhost:8890'

export default defineConfig({
  plugins: [
    tanstackRouter({ target: 'react', autoCodeSplitting: true, quoteStyle: 'single', semicolons: false }),
    viteReact(),
    tailwindcss(),
  ],
  resolve: { alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) } },
  server: {
    proxy: {
      '/api': bot,
      '/login': bot,
      '/logout': bot,
      '/auth': bot,
    },
  },
  build: { outDir: 'dist', emptyOutDir: true, chunkSizeWarningLimit: 400 },
})
