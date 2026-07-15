import { mkdir, readFile, realpath, rename, rm, stat, writeFile } from 'node:fs/promises'
import { randomUUID } from 'node:crypto'
import { isAbsolute, join, parse, relative, resolve, sep } from 'node:path'

export const workspaceRecordName = 'workspace.json'

export interface WorkspaceAuthorizationOptions {
  readonly userDataPath: string
  readonly homePath: string
  readonly applicationPaths: readonly string[]
  readonly selectDirectory: () => Promise<string | undefined>
}

export type WorkspaceAuthorization =
  | { readonly ok: true; readonly path: string }
  | { readonly ok: false; readonly reason: string }

export async function authorizePackagedWorkspace(
  options: WorkspaceAuthorizationOptions,
): Promise<WorkspaceAuthorization> {
  const recordPath = join(options.userDataPath, workspaceRecordName)
  const storedPath = await readRecord(recordPath)
  if (storedPath) {
    const stored = await validateWorkspaceSelection(storedPath, options)
    if (stored) return { ok: true, path: stored }
  }

  let selected: string | undefined
  try {
    selected = await options.selectDirectory()
  } catch {
    return failure('Workspace selection failed.')
  }
  if (!selected) return failure('Workspace selection was cancelled.')

  const workspace = await validateWorkspaceSelection(selected, options)
  if (!workspace) return failure('The selected workspace is unavailable or unsafe.')
  try {
    await persistRecord(recordPath, workspace)
  } catch {
    return failure('The workspace authorization could not be saved.')
  }
  return { ok: true, path: workspace }
}

async function readRecord(path: string): Promise<string | undefined> {
  try {
    const value: unknown = JSON.parse(await readFile(path, 'utf8'))
    if (!value || typeof value !== 'object') return undefined
    const record = value as Record<string, unknown>
    return record.version === 1 && typeof record.path === 'string'
      ? record.path
      : undefined
  } catch {
    return undefined
  }
}

export async function validateWorkspaceSelection(
  candidate: string,
  options: Omit<WorkspaceAuthorizationOptions, 'selectDirectory'>,
): Promise<string | undefined> {
  let canonical: string
  try {
    canonical = await realpath(candidate)
    if (!(await stat(canonical)).isDirectory()) return undefined
  } catch {
    return undefined
  }
  if (samePath(canonical, parse(canonical).root)) return undefined
  const home = await canonicalReference(options.homePath)
  if (containsPath(canonical, home)) return undefined
  const reserved = [options.userDataPath, ...options.applicationPaths]
  for (const path of reserved) {
    const reference = await canonicalReference(path)
    if (containsPath(canonical, reference) || containsPath(reference, canonical)) {
      return undefined
    }
  }
  return canonical
}

async function canonicalReference(path: string): Promise<string> {
  try {
    return await realpath(path)
  } catch {
    return resolve(path)
  }
}

function containsPath(parent: string, child: string): boolean {
  const value = relative(normalizePath(parent), normalizePath(child))
  return value === '' || (value !== '..' && !value.startsWith(`..${sep}`) && !isAbsolute(value))
}

function samePath(left: string, right: string): boolean {
  return normalizePath(left) === normalizePath(right)
}

function normalizePath(path: string): string {
  return resolve(path)
}

async function persistRecord(recordPath: string, workspace: string): Promise<void> {
  const directory = resolve(recordPath, '..')
  const temporary = join(directory, `.workspace-${randomUUID()}.tmp`)
  await mkdir(directory, { recursive: true })
  let committed = false
  try {
    await writeFile(temporary, `${JSON.stringify({ version: 1, path: workspace })}\n`, {
      encoding: 'utf8',
      flag: 'wx',
      mode: 0o600,
    })
    await rename(temporary, recordPath)
    committed = true
  } finally {
    if (!committed) await rm(temporary, { force: true })
  }
}

function failure(reason: string): WorkspaceAuthorization {
  return { ok: false, reason }
}
