import { chmod, cp, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export async function loadLlmScenario(
  name: string,
  variables: Readonly<Record<string, string>>,
): Promise<unknown> {
  if (!/^[a-z0-9-]+$/.test(name)) throw new Error(`Invalid fixture name: ${name}`)
  const content = await readFile(fixturePath(`llm/${name}.json`), 'utf8')
  return interpolate(JSON.parse(content) as unknown, variables)
}

export async function seedWorkspace(
  project: string, root: string, fixture = 'basic',
): Promise<void> {
  if (!/^[a-z0-9-]+$/.test(fixture)) throw new Error(`Invalid workspace fixture: ${fixture}`)
  await cp(fixturePath(`workspaces/${fixture}`), project, { recursive: true, dereference: true })
  await cp(fixturePath('outside.txt'), join(root, 'outside.txt'), { dereference: true })
  const encodedImage = join(project, 'pixel.png.base64')
  const image = Buffer.from((await readFile(encodedImage, 'utf8')).trim(), 'base64')
  await writeFile(join(project, 'pixel.png'), image)
  await rm(encodedImage)
  await makeWritable(project)
  await chmod(join(root, 'outside.txt'), 0o644)
}

export async function seedPlugin(home: string, fixture: string): Promise<string> {
  if (!/^[a-z0-9-]+$/.test(fixture)) throw new Error(`Invalid plugin fixture: ${fixture}`)
  const target = join(home, '.loopal', 'plugins', fixture)
  await mkdir(dirname(target), { recursive: true })
  await cp(fixturePath(`plugins/${fixture}`), target, { recursive: true, dereference: true })
  await makeWritable(target)
  return target
}

async function makeWritable(path: string): Promise<void> {
  await chmod(path, 0o755)
  const entries = await readdir(path, { withFileTypes: true })
  await Promise.all(entries.map(async (entry) => {
    const child = join(path, entry.name)
    if (entry.isDirectory()) await makeWritable(child)
    else if (entry.isFile()) await chmod(child, 0o644)
  }))
}

export function fixturePath(relative: string): string {
  const testSrcDir = process.env.TEST_SRCDIR
  const workspace = process.env.TEST_WORKSPACE
  if (testSrcDir && workspace) {
    return join(testSrcDir, workspace, 'apps/desktop/e2e/fixtures', relative)
  }
  return resolve(dirname(fileURLToPath(import.meta.url)), '../../fixtures', relative)
}

function interpolate(value: unknown, variables: Readonly<Record<string, string>>): unknown {
  if (typeof value === 'string') {
    const resolved = value.replace(/\$\{([A-Z_]+)\}/g, (_, name: string) => {
      const replacement = variables[name]
      if (replacement === undefined) throw new Error(`Unknown fixture variable: ${name}`)
      return replacement
    })
    if (resolved.includes('${')) throw new Error(`Unresolved fixture variable in: ${resolved}`)
    return resolved
  }
  if (Array.isArray(value)) return value.map((item) => interpolate(item, variables))
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => (
      [key, interpolate(item, variables)]
    )))
  }
  return value
}
