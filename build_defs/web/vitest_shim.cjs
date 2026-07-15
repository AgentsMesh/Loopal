#!/usr/bin/env node

// Resolve Vitest from the Bazel-created node_modules tree instead of relying
// on a source-tree executable or an npm/pnpm script. The Node option is
// inherited by Vitest workers and permits Vite's ESM-only dependencies to be
// loaded by the small CommonJS compatibility shims shipped by Vitest/jsdom.
const { spawnSync } = require('node:child_process')
const { existsSync, readFileSync } = require('node:fs')
const { createRequire } = require('node:module')
const path = require('node:path')

const workdir = resolveRunfile(process.env.VITEST_RUNFILES_WORKDIR)
process.chdir(workdir)
const requireFromCwd = createRequire(path.join(workdir, 'noop.cjs'))
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
    cwd: workdir,
    env: { ...process.env, NODE_OPTIONS: nodeOptions },
    stdio: 'inherit',
  },
)

process.exit(result.status ?? 1)

function resolveRunfile(logicalPath) {
  if (!logicalPath) return process.cwd()
  const root = process.env.RUNFILES_DIR
    ?? process.env.JS_BINARY__RUNFILES
    ?? process.env.TEST_SRCDIR
  const candidate = root && path.join(root, logicalPath)
  if (candidate && existsSync(candidate)) return candidate
  const manifest = process.env.RUNFILES_MANIFEST_FILE
  if (manifest && existsSync(manifest)) {
    const match = readManifest(manifest, logicalPath)
    if (match) return match
  }
  throw new Error(`Bazel runfile not found: ${logicalPath}`)
}

function readManifest(manifest, logicalPath) {
  for (const line of readFileSync(manifest, 'utf8').split(/\r?\n/u)) {
    const escaped = line.startsWith(' ')
    const separator = line.indexOf(' ', escaped ? 1 : 0)
    if (separator < 0) continue
    const decode = (value) => escaped
      ? value.replace(/\\([snb])/gu, (_, code) => ({ s: ' ', n: '\n', b: '\\' })[code])
      : value
    if (decode(line.slice(escaped ? 1 : 0, separator)) === logicalPath) {
      return decode(line.slice(separator + 1))
    }
  }
  return undefined
}
