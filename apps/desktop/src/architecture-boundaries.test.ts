import { readdirSync, readFileSync, statSync } from 'node:fs'
import { dirname, extname, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

const sourceRoot = dirname(fileURLToPath(import.meta.url))
const modulePattern = /\b(?:from|import)\s*(?:\(\s*)?['"](\.\.?\/[^'"\n]+)['"]/g

function filesBelow(root: string): string[] {
  return readdirSync(root).flatMap((name) => {
    const path = join(root, name)
    return statSync(path).isDirectory() ? filesBelow(path) : [path]
  })
}

function sourcePath(path: string): string {
  return relative(sourceRoot, path).split(sep).join('/')
}

function lineCount(path: string): number {
  const source = readFileSync(path, 'utf8')
  return source.split('\n').length - Number(source.endsWith('\n'))
}

function resolveModule(importer: string, specifier: string): string | undefined {
  const base = resolve(dirname(importer), specifier)
  return [
    base,
    `${base}.ts`,
    `${base}.tsx`,
    `${base}.css`,
    join(base, 'index.ts'),
    join(base, 'index.tsx'),
  ].find((candidate) => {
    try {
      return statSync(candidate).isFile()
    } catch {
      return false
    }
  })
}

function relativeDependencies(path: string): string[] {
  const source = readFileSync(path, 'utf8')
  return [...source.matchAll(modulePattern)]
    .map((match) => match[1] ? resolveModule(path, match[1]) : undefined)
    .filter((target): target is string => Boolean(target))
}

function forbiddenDependency(importer: string, target: string): boolean {
  if (importer.startsWith('base/')) return !target.startsWith('base/')
  if (importer.startsWith('shared/')) return !target.startsWith('shared/')
  if (importer.startsWith('workbench/')) {
    return target.startsWith('main/')
      || target.startsWith('preload/')
      || /^platform\/[^/]+\/node\//.test(target)
  }
  if (importer.startsWith('preload/')) {
    return target.startsWith('main/')
      || target.startsWith('renderer/')
      || target.startsWith('workbench/')
      || /^platform\/[^/]+\/node\//.test(target)
  }
  if (/^platform\/[^/]+\/common\//.test(importer)) {
    return /^platform\/[^/]+\/node\//.test(target)
      || target.startsWith('main/')
      || target.startsWith('workbench/')
  }
  return false
}

describe('Desktop source architecture', () => {
  const sources = filesBelow(sourceRoot)

  it('keeps imports inside the intended dependency direction', () => {
    const violations = sources
      .filter((path) => ['.ts', '.tsx'].includes(extname(path)))
      .flatMap((path) => relativeDependencies(path).map((target) => [path, target] as const))
      .filter(([, target]) => target.startsWith(sourceRoot))
      .map(([path, target]) => [sourcePath(path), sourcePath(target)] as const)
      .filter(([importer, target]) => forbiddenDependency(importer, target))
      .map(([importer, target]) => `${importer} -> ${target}`)

    expect(violations).toEqual([])
  })

  it('keeps platform implementation roots grouped by responsibility', () => {
    const roots = [
      'platform/desktop-host/node',
      'platform/loopal-backend/node',
    ]
    const flatFiles = roots.flatMap((root) => readdirSync(join(sourceRoot, root))
      .filter((name) => statSync(join(sourceRoot, root, name)).isFile())
      .map((name) => `${root}/${name}`))

    expect(flatFiles).toEqual([])
    expect(readdirSync(join(sourceRoot, 'shared')).sort()).toEqual([
      'contracts',
      'global.d.ts',
      'i18n',
      'protocol',
    ])
  })

  it('keeps handwritten Desktop source files within 200 lines', () => {
    const oversized = sources
      .filter((path) => ['.ts', '.tsx', '.css'].includes(extname(path)))
      .map((path) => [
        sourcePath(path),
        lineCount(path),
      ] as const)
      .filter(([, lines]) => lines > 200)
      .map(([path, lines]) => `${path}: ${lines}`)

    expect(oversized).toEqual([])
  })
})
