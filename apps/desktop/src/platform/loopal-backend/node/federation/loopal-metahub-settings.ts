import { randomUUID } from 'node:crypto'
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'
import { z } from 'zod'
import {
  MetaHubSettingsSchema,
  UpdateMetaHubSettingsInputSchema,
  type MetaHubSettings,
  type UpdateMetaHubSettingsInput,
} from '../../../../shared/contracts'
import { type MetaHubStartupOptions } from '../../../desktop-host/node/host/desktop-host-types'

const StoredSchema = MetaHubSettingsSchema.omit({ tokenConfigured: true }).extend({
  version: z.literal(1),
  token: z.string().max(4096).optional(),
})
type Stored = z.infer<typeof StoredSchema>

export class LoopalMetaHubSettings {
  private value: Stored = {
    version: 1,
    address: '',
    hubName: `desktop-${randomUUID().slice(0, 8)}`,
    joinOnStart: false,
    startLocalOnLaunch: false,
  }
  private writes = Promise.resolve()

  constructor(private readonly path?: string) {}

  get publicValue(): MetaHubSettings {
    const { token: _token, version: _version, ...value } = this.value
    return { ...value, tokenConfigured: Boolean(this.value.token) }
  }

  get startup(): MetaHubStartupOptions | undefined {
    if (!this.value.joinOnStart || !this.value.address || !this.value.token) return undefined
    return {
      address: this.value.address,
      hubName: this.value.hubName,
      token: this.value.token,
    }
  }

  get credentials(): MetaHubStartupOptions | undefined {
    if (!this.value.address || !this.value.token) return undefined
    return {
      address: this.value.address,
      hubName: this.value.hubName,
      token: this.value.token,
    }
  }

  async load(): Promise<void> {
    if (!this.path) return
    try {
      this.value = StoredSchema.parse(JSON.parse(await readFile(this.path, 'utf8')))
    } catch (error) {
      if (!isMissing(error)) await this.persist()
    }
  }

  async update(input: UpdateMetaHubSettingsInput): Promise<MetaHubSettings> {
    const next = UpdateMetaHubSettingsInputSchema.parse(input)
    const token = next.clearToken ? undefined : next.token ?? this.value.token
    this.value = {
      version: 1,
      address: next.address,
      hubName: next.hubName,
      joinOnStart: next.joinOnStart,
      startLocalOnLaunch: next.startLocalOnLaunch,
      ...(token ? { token } : {}),
    }
    await this.persist()
    return this.publicValue
  }

  async useManaged(address: string, token: string): Promise<void> {
    this.value = { ...this.value, address, token }
    await this.persist()
  }

  async clearManaged(address: string): Promise<boolean> {
    if (this.value.address !== address) return false
    this.value = {
      version: 1,
      address: '',
      hubName: this.value.hubName,
      joinOnStart: false,
      startLocalOnLaunch: this.value.startLocalOnLaunch,
    }
    await this.persist()
    return true
  }

  flush(): Promise<void> { return this.writes }

  private persist(): Promise<void> {
    if (!this.path) return Promise.resolve()
    const snapshot = JSON.stringify(this.value)
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

function isMissing(error: unknown): boolean {
  return error instanceof Error && 'code' in error && error.code === 'ENOENT'
}
