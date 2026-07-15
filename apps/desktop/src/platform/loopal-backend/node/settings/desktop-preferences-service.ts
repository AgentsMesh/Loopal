import { randomUUID } from 'node:crypto'
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'
import { z } from 'zod'
import {
  DEFAULT_DESKTOP_PREFERENCES,
  DesktopPreferencesSchema,
  UpdateDesktopPreferencesInputSchema,
  type DesktopPreferences,
  type UpdateDesktopPreferencesInput,
} from '../../../../shared/contracts'

const StoredSchema = DesktopPreferencesSchema.extend({ version: z.literal(1) })

export interface DesktopPreferencesOperations {
  getDesktopPreferences(): Promise<DesktopPreferences>
  updateDesktopPreferences(input: UpdateDesktopPreferencesInput): Promise<DesktopPreferences>
}

export class DesktopPreferencesService implements DesktopPreferencesOperations {
  private value = DEFAULT_DESKTOP_PREFERENCES
  private loaded = false
  private loading?: Promise<void>
  private writes = Promise.resolve()

  constructor(private readonly path?: string) {}

  async getDesktopPreferences(): Promise<DesktopPreferences> {
    await this.load()
    return { ...this.value }
  }

  async updateDesktopPreferences(
    input: UpdateDesktopPreferencesInput,
  ): Promise<DesktopPreferences> {
    await this.load()
    this.value = UpdateDesktopPreferencesInputSchema.parse(input)
    await this.persist()
    return { ...this.value }
  }

  flush(): Promise<void> { return this.writes }

  private load(): Promise<void> {
    if (this.loaded) return Promise.resolve()
    this.loading ??= this.loadInner()
    return this.loading
  }

  private async loadInner(): Promise<void> {
    try {
      if (this.path) {
        const stored = StoredSchema.parse(JSON.parse(await readFile(this.path, 'utf8')))
        this.value = { locale: stored.locale }
      }
    } catch (error) {
      if (!isMissing(error)) await this.persist()
    } finally {
      this.loaded = true
    }
  }

  private persist(): Promise<void> {
    if (!this.path) return Promise.resolve()
    const snapshot = JSON.stringify({ version: 1, ...this.value })
    const write = async (): Promise<void> => {
      await mkdir(dirname(this.path!), { recursive: true })
      const temporary = `${this.path}.${process.pid}.${randomUUID()}.tmp`
      try {
        await writeFile(temporary, snapshot, { encoding: 'utf8', mode: 0o600 })
        await rename(temporary, this.path!)
      } finally {
        await rm(temporary, { force: true }).catch(() => undefined)
      }
    }
    const queued = this.writes.then(write, write)
    this.writes = queued
    return queued
  }
}

export function bindDesktopPreferences(
  service: DesktopPreferencesOperations,
): DesktopPreferencesOperations {
  return {
    getDesktopPreferences: service.getDesktopPreferences.bind(service),
    updateDesktopPreferences: service.updateDesktopPreferences.bind(service),
  }
}

function isMissing(error: unknown): boolean {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT'
}
