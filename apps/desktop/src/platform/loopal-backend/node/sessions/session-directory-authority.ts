import { randomUUID } from 'node:crypto'
import { basename } from 'node:path'
import { z } from 'zod'
import {
  type CreateSessionInput, type SessionDirectorySelection,
  SessionDirectorySelectionSchema,
} from '../../../../shared/contracts'

const InspectionSchema = z.object({
  path: z.string().min(1),
  name: z.string().min(1),
  git: z.object({
    root: z.string().min(1), branch: z.string().min(1).optional(),
    head: z.string().regex(/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/u).optional(), dirty: z.boolean(),
  }).strict().optional(),
}).strict()
const PreparedSchema = z.object({
  path: z.string().min(1), branch: z.string().min(1), name: z.string().min(1),
}).strict()
const CleanupSchema = z.object({ path: z.string().min(1), removed: z.literal(true) }).strict()
type Inspection = z.infer<typeof InspectionSchema>

interface Authorization {
  readonly value: Inspection
  readonly expiresAt: number
}

export interface PreparedSessionDirectory {
  readonly path: string
  readonly kind: 'folder' | 'git_worktree'
  readonly branch?: string
}
export interface SessionDirectoryClaim {
  readonly target: PreparedSessionDirectory
  commit(): void
  rollback(): Promise<void>
}
export type SessionDirectoryRequest = (method: string, params: unknown) => Promise<unknown>

export class SessionDirectoryAuthority {
  private readonly entries = new Map<string, Authorization>()

  constructor(
    private readonly request: SessionDirectoryRequest,
    private readonly now: () => number = Date.now,
    private readonly ttlMs = 10 * 60_000,
  ) {}

  async authorize(path: string): Promise<SessionDirectorySelection> {
    const value = InspectionSchema.parse(await this.request(
      'desktop/inspectWorkingDirectory', { path },
    ))
    this.prune()
    const authorizationId = randomUUID()
    this.entries.set(authorizationId, { value, expiresAt: this.now() + this.ttlMs })
    while (this.entries.size > 32) this.entries.delete(this.entries.keys().next().value!)
    const git = value.git ? {
      root: value.git.root, dirty: value.git.dirty,
      ...(value.git.branch ? { branch: value.git.branch } : {}),
    } : undefined
    return SessionDirectorySelectionSchema.parse({
      authorizationId,
      path: value.path, name: value.name, ...(git ? { git } : {}),
      suggestedWorktreeName: suggestedName(value.name, this.now()),
    })
  }

  async prepare(input: CreateSessionInput): Promise<PreparedSessionDirectory> {
    const claim = await this.claim(input)
    claim.commit()
    return claim.target
  }

  async claim(input: CreateSessionInput): Promise<SessionDirectoryClaim> {
    this.prune()
    const entry = this.entries.get(input.authorizationId)
    if (!entry) throw coded('directory_authorization_invalid', 'directory authorization expired')
    if (input.launchMode === 'worktree' && !entry.value.git) {
      throw coded('not_git_repository', 'worktree mode requires Git')
    }
    this.entries.delete(input.authorizationId)
    if (input.launchMode === 'directory') {
      try {
        await this.revalidate(entry.value)
        return this.transaction(
          input.authorizationId, entry, { path: entry.value.path, kind: 'folder' },
        )
      } catch (error) {
        if (entry.expiresAt > this.now()) this.restore(input.authorizationId, entry)
        throw error
      }
    }
    const git = entry.value.git!
    try {
      const prepared = PreparedSchema.parse(await this.request('desktop/prepareWorktree', {
        path: entry.value.path, name: input.worktreeName,
        expectedRoot: git.root, expectedHead: git.head ?? 'UNBORN',
      }))
      return this.transaction(input.authorizationId, entry, {
        path: prepared.path, kind: 'git_worktree', branch: prepared.branch,
      }, async () => {
        CleanupSchema.parse(await this.request('desktop/cleanupWorktree', {
          path: entry.value.path, name: prepared.name, expectedPath: prepared.path,
        }))
      })
    } catch (error) {
      if (!isRetainedWorktree(error) && entry.expiresAt > this.now()) {
        this.restore(input.authorizationId, entry)
      }
      throw error
    }
  }

  private async revalidate(expected: Inspection): Promise<void> {
    const current = InspectionSchema.parse(await this.request(
      'desktop/inspectWorkingDirectory', { path: expected.path },
    ))
    if (current.path !== expected.path || current.git?.root !== expected.git?.root) {
      throw coded(
        'working_directory_changed', 'working directory changed; select it again',
      )
    }
  }

  private transaction(
    id: string,
    entry: Authorization,
    target: PreparedSessionDirectory,
    cleanup?: () => Promise<void>,
  ): SessionDirectoryClaim {
    let settled = false
    return {
      target,
      commit: () => { settled = true },
      rollback: async () => {
        if (settled) return
        settled = true
        await cleanup?.()
        if (entry.expiresAt > this.now()) this.restore(id, entry)
      },
    }
  }

  private prune(): void {
    const now = this.now()
    for (const [id, value] of this.entries) {
      if (value.expiresAt <= now) this.entries.delete(id)
    }
  }

  private restore(id: string, entry: Authorization): void {
    this.entries.set(id, entry)
    while (this.entries.size > 32) this.entries.delete(this.entries.keys().next().value!)
  }
}

function suggestedName(name: string, now: number): string {
  const clean = basename(name).replace(/[^A-Za-z0-9_-]+/gu, '-').replace(/^-+/u, '')
  const stamp = new Date(now).toISOString().replace(/[-:TZ.]/gu, '').slice(0, 14)
  return `${(clean || 'loopal').slice(0, 43)}-${stamp}`.slice(0, 64)
}

function coded(code: string, message: string): Error {
  return new Error(`${code}: ${message}`)
}

function isRetainedWorktree(error: unknown): boolean {
  return error instanceof Error && error.message.startsWith('worktree_retained:')
}
