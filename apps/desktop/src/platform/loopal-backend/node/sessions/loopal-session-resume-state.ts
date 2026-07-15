import { randomUUID } from 'node:crypto'
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'
import { z } from 'zod'

const SessionLocationSchema = z.object({
  sessionId: z.string().min(1),
  workspaceId: z.string().min(1),
  cwd: z.string().min(1),
  name: z.string().min(1),
  kind: z.enum(['folder', 'git_worktree']),
  title: z.string().min(1).optional(),
  model: z.string().min(1).optional(),
  mode: z.string().min(1).optional(),
  createdAt: z.string().datetime().optional(),
  updatedAt: z.string().datetime().optional(),
})
export type SessionLocation = z.infer<typeof SessionLocationSchema>

const PersistedStateSchema = z.object({
  version: z.literal(2),
  workspace: z.string().min(1),
  activeSessionId: z.string().min(1).optional(),
  runningSessionIds: z.array(z.string().min(1)).max(32),
  sessionLocations: z.array(SessionLocationSchema).max(128),
})
const LegacyStateSchema = z.object({
  version: z.literal(1), workspace: z.string().min(1),
  activeSessionId: z.string().min(1).optional(),
  runningSessionIds: z.array(z.string().min(1)).max(32),
})

type PersistedState = z.infer<typeof PersistedStateSchema>

export class LoopalSessionResumeState {
  private value: PersistedState
  private writes = Promise.resolve()

  constructor(
    private readonly workspace: string,
    private readonly path?: string,
  ) {
    this.value = { version: 2, workspace, runningSessionIds: [], sessionLocations: [] }
  }

  get activeSessionId(): string | undefined { return this.value.activeSessionId }
  get runningSessionIds(): readonly string[] { return this.value.runningSessionIds }
  get resumeSessionId(): string | undefined {
    const active = this.value.activeSessionId
    return active && this.value.runningSessionIds.includes(active)
      ? active
      : this.value.runningSessionIds[0]
  }
  get locations(): readonly SessionLocation[] { return this.value.sessionLocations }
  location(sessionId: string): SessionLocation | undefined {
    return this.value.sessionLocations.find((value) => value.sessionId === sessionId)
  }

  async load(): Promise<void> {
    if (!this.path) return
    try {
      const raw: unknown = JSON.parse(await readFile(this.path, 'utf8'))
      const current = PersistedStateSchema.safeParse(raw)
      const legacy = LegacyStateSchema.safeParse(raw)
      const parsed = current.success ? current.data : legacy.success ? {
        ...legacy.data, version: 2 as const, sessionLocations: [],
      } : undefined
      if (parsed?.workspace === this.workspace) this.value = parsed
    } catch (error) {
      if (!isMissing(error)) this.value = this.empty()
    }
  }

  select(sessionId: string): Promise<void> {
    this.value = { ...this.value, activeSessionId: sessionId }
    return this.persist()
  }

  created(location: SessionLocation): Promise<void> {
    this.value = {
      ...this.value,
      sessionLocations: [
        location,
        ...this.value.sessionLocations.filter((value) => value.sessionId !== location.sessionId),
      ].slice(0, 128),
    }
    return this.persist()
  }

  started(sessionId: string, select: boolean, location?: SessionLocation): Promise<void> {
    const runningSessionIds = [
      sessionId, ...this.value.runningSessionIds.filter((id) => id !== sessionId),
    ].slice(0, 32)
    this.value = {
      ...this.value,
      runningSessionIds,
      sessionLocations: location ? [
        location,
        ...this.value.sessionLocations.filter((value) => value.sessionId !== sessionId),
      ].slice(0, 128) : this.value.sessionLocations,
      ...(select ? { activeSessionId: sessionId } : {}),
    }
    return this.persist()
  }

  stopped(sessionId: string): Promise<void> {
    this.value = {
      ...this.value,
      runningSessionIds: this.value.runningSessionIds.filter((id) => id !== sessionId),
    }
    return this.persist()
  }

  discardUnavailable(sessionIds: readonly string[]): Promise<void> {
    const unavailable = new Set(sessionIds)
    this.value = {
      ...this.value,
      runningSessionIds: this.value.runningSessionIds.filter((id) => !unavailable.has(id)),
      ...(this.value.activeSessionId && unavailable.has(this.value.activeSessionId)
        ? { activeSessionId: undefined } : {}),
    }
    return this.persist()
  }

  normalizeRunning(sessionIds: readonly string[], activeSessionId: string): Promise<void> {
    const runningSessionIds = [...new Set(sessionIds)].slice(0, 32)
    this.value = { ...this.value, runningSessionIds, activeSessionId }
    return this.persist()
  }

  flush(): Promise<void> { return this.writes }

  private persist(): Promise<void> {
    if (!this.path) return Promise.resolve()
    const snapshot = JSON.stringify(this.value)
    const persist = async (): Promise<void> => {
      await mkdir(dirname(this.path!), { recursive: true })
      const temporary = `${this.path}.${process.pid}.${randomUUID()}.tmp`
      try {
        await writeFile(temporary, snapshot, { encoding: 'utf8', mode: 0o600 })
        await rename(temporary, this.path!)
      } finally {
        try { await rm(temporary, { force: true }) }
        catch {}
      }
    }
    const write = this.writes.then(persist, persist)
    this.writes = write
    return write
  }

  private empty(): PersistedState {
    return { version: 2, workspace: this.workspace, runningSessionIds: [], sessionLocations: [] }
  }
}

function isMissing(error: unknown): boolean {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT'
}
