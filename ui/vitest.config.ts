import { defineConfig, mergeConfig } from 'vitest/config'

import viteConfig from './vite.config.ts'

// Mêmes extensions que l'application (routeur, React Compiler, alias) : on teste ce qui est livré.
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'happy-dom',
      setupFiles: ['src/test/setup.ts'],
      include: ['src/**/*.test.{ts,tsx}'],
      restoreMocks: true,
      // La maquette simule 120 à 370 ms de latence par requête.
      testTimeout: 15_000,
      unstubEnvs: true,
      unstubGlobals: true,
      coverage: {
        provider: 'v8',
        include: ['src/**/*.{ts,tsx}'],
        exclude: [
          'src/**/*.test.{ts,tsx}',
          'src/test/**',
          'src/api/generated/**',
          'src/routeTree.gen.ts',
          'src/mocks/**',
        ],
        reporter: ['text-summary', 'html'],
      },
    },
  }),
)
