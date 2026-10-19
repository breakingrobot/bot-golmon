// Vérifie que le React Compiler compile chaque composant et hook de src/ (aucun abandon silencieux).
// Usage : node scripts/check-compiler.mjs (aussi `npm run check:compiler`).

import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

import { transformSync } from '@babel/core'
import reactCompiler from 'babel-plugin-react-compiler'

const root = fileURLToPath(new URL('../src', import.meta.url))
const skipped = ['routeTree.gen.ts', 'api/generated', 'api/contract-check.ts']

const files = (dir) =>
  readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    if (skipped.some((skip) => relative(root, path).startsWith(skip))) return []
    if (statSync(path).isDirectory()) return files(path)
    return /\.(ts|tsx)$/.test(name) && !/\.test\.tsx?$/.test(name) ? [path] : []
  })

const problems = []
let compiled = 0
for (const file of files(root)) {
  transformSync(readFileSync(file, 'utf8'), {
    filename: file,
    babelrc: false,
    configFile: false,
    parserOpts: { plugins: ['typescript', 'jsx'] },
    plugins: [
      [
        reactCompiler,
        {
          target: '19',
          logger: {
            logEvent: (filename, event) => {
              if (event.kind === 'CompileSuccess') compiled += 1
              if (event.kind === 'CompileError' || event.kind === 'PipelineError') {
                const detail = event.detail?.reason ?? event.detail?.options?.reason ?? event.data ?? ''
                problems.push(`${relative(root, filename ?? file)} : ${event.kind} ${String(detail)}`)
              }
            },
          },
        },
      ],
    ],
  })
}

process.stdout.write(`React Compiler : ${compiled} fonctions compilées.\n`)
if (problems.length > 0) {
  console.error(problems.join('\n'))
  process.exit(1)
}
