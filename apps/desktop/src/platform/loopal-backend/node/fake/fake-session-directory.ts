import { execFile } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { mkdir, realpath, stat } from 'node:fs/promises'
import { basename, join, relative } from 'node:path'
import { promisify } from 'node:util'
import {
  type CreateSessionInput, type SessionDirectorySelection,
  SessionDirectorySelectionSchema,
} from '../../../../shared/contracts'
import { type PreparedSessionDirectory } from '../sessions/session-directory-authority'

const execute = promisify(execFile)

interface Entry {
  readonly path: string
  readonly git?: { readonly root: string; readonly branch?: string; readonly dirty: boolean }
}

export class FakeSessionDirectoryAuthority {
  private readonly entries = new Map<string, Entry>()

  async authorize(candidate: string): Promise<SessionDirectorySelection> {
    const path = await realpath(candidate)
    if (!(await stat(path)).isDirectory() || !basename(path)) {
      throw new Error('unsafe_working_directory: directory required')
    }
    const git = await inspectGit(path)
    const authorizationId = randomUUID()
    this.entries.set(authorizationId, { path, ...(git ? { git } : {}) })
    return SessionDirectorySelectionSchema.parse({
      authorizationId, path, name: basename(path), git,
      suggestedWorktreeName: `loopal-${Date.now()}`,
    })
  }

  async prepare(input: CreateSessionInput)
    : Promise<PreparedSessionDirectory> {
    const selected = this.entries.get(input.authorizationId)
    if (!selected) throw new Error('directory_authorization_invalid: selection expired')
    if (input.launchMode === 'directory') {
      this.entries.delete(input.authorizationId)
      return { path: selected.path, kind: 'folder' }
    }
    if (!selected.git) throw new Error('not_git_repository: worktree mode requires Git')
    this.entries.delete(input.authorizationId)
    const root = join(selected.git.root, '.loopal', 'worktrees', input.worktreeName)
    await mkdir(join(selected.git.root, '.loopal', 'worktrees'), { recursive: true })
    try {
      await execute('git', [
        'worktree', 'add', '-b', `loopal-wt-${input.worktreeName}`, root,
      ], { cwd: selected.git.root })
    } catch (error) {
      this.entries.set(input.authorizationId, selected)
      throw new Error(`worktree_creation_failed: ${errorMessage(error)}`)
    }
    return {
      path: join(root, relative(selected.git.root, selected.path)),
      kind: 'git_worktree', branch: `loopal-wt-${input.worktreeName}`,
    }
  }
}

async function inspectGit(path: string): Promise<Entry['git']> {
  try {
    const root = (await execute('git', ['rev-parse', '--show-toplevel'], { cwd: path }))
      .stdout.trim()
    const branchValue = (await execute('git', ['branch', '--show-current'], { cwd: path }))
      .stdout.trim()
    const dirty = (await execute('git', ['status', '--porcelain'], { cwd: path }))
      .stdout.trim().length > 0
    return { root: await realpath(root), ...(branchValue ? { branch: branchValue } : {}), dirty }
  } catch { return undefined }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
