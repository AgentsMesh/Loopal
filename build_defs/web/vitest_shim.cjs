#!/usr/bin/env node

// Resolve Vitest from the Bazel-created node_modules tree instead of relying
// on a source-tree executable or an npm/pnpm script. The Node option is
// inherited by Vitest workers and permits Vite's ESM-only dependencies to be
// loaded by the small CommonJS compatibility shims shipped by Vitest/jsdom.
const { spawnSync } = require('node:child_process')
const { createRequire } = require('node:module')
const path = require('node:path')

const requireFromCwd = createRequire(path.join(process.cwd(), 'noop.cjs'))
const packageJsonPath = requireFromCwd.resolve('vitest/package.json')
const binaryPath = path.join(path.dirname(packageJsonPath), 'vitest.mjs')
const nodeOptions = [
  process.env.NODE_OPTIONS ?? '',
  '--experimental-require-module',
]
  .filter(Boolean)
  .join(' ')

const result = spawnSync(
  process.execPath,
  [binaryPath, ...process.argv.slice(2)],
  {
    cwd: process.cwd(),
    env: { ...process.env, NODE_OPTIONS: nodeOptions },
    stdio: 'inherit',
  },
)

process.exit(result.status ?? 1)
