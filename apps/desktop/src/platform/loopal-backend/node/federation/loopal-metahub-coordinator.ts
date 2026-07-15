import {
  spawn,
  type ChildProcessByStdio,
} from 'node:child_process'
import { createInterface } from 'node:readline'
import { type Readable } from 'node:stream'
import { z } from 'zod'
import { type IDisposable } from '../../../../base/common/lifecycle'
import { type LocalMetaHubStatus } from '../../../../shared/contracts'

const PREFIX = 'LOOPAL_METAHUB '
const HandshakeSchema = z.object({
  protocol_version: z.literal(1),
  phase: z.literal('ready'),
  address: z.string().min(1),
  token: z.string().min(1),
  pid: z.number().int().positive(),
  parent_pid: z.number().int().positive(),
})

export interface ManagedMetaHub {
  readonly address: string
  readonly token: string
}
type MetaHubChild = ChildProcessByStdio<null, Readable, Readable>
export type SpawnMetaHubProcess = (
  binary: string,
  args: readonly string[],
) => MetaHubChild

export class LoopalMetaHubCoordinator implements IDisposable {
  private child: MetaHubChild | undefined
  private ready: ManagedMetaHub | undefined
  private pending: Promise<ManagedMetaHub> | undefined
  private stopping: Promise<void> | undefined
  private failure: string | undefined
  private managedAddress: string | undefined
  private generation = 0

  constructor(
    private readonly binaryPath: string,
    private readonly parentPid: number,
    private readonly spawnProcess: SpawnMetaHubProcess = defaultSpawn,
  ) {}

  get status(): LocalMetaHubStatus {
    if (this.ready) return { state: 'running', address: this.ready.address }
    if (this.pending) return { state: 'starting' }
    if (this.failure) return { state: 'failed', error: this.failure }
    return { state: 'stopped' }
  }

  get ownedAddress(): string | undefined { return this.managedAddress }

  start(bindAddress: string): Promise<ManagedMetaHub> {
    if (this.ready) return Promise.resolve(this.ready)
    if (this.stopping) return this.stopping.then(() => this.start(bindAddress))
    if (this.pending) return this.pending
    const generation = ++this.generation
    const pending = this.startProcess(bindAddress, generation)
    const tracked = pending.finally(() => {
      if (this.pending === tracked) this.pending = undefined
    })
    this.pending = tracked
    return this.pending
  }

  stop(): Promise<void> {
    if (this.stopping) return this.stopping
    const generation = ++this.generation
    const stopping = this.stopInternal(generation)
    const tracked = stopping.finally(() => {
      if (this.stopping === tracked) this.stopping = undefined
    })
    this.stopping = tracked
    return this.stopping
  }

  private async stopInternal(_generation: number): Promise<void> {
    const child = this.child
    this.child = undefined
    this.ready = undefined
    this.failure = undefined
    this.managedAddress = undefined
    if (child && child.exitCode === null) {
      child.kill('SIGTERM')
      const exited = new Promise<void>((resolve) => child.once('exit', () => resolve()))
      let timer: NodeJS.Timeout | undefined
      const timeout = new Promise<void>((resolve) => { timer = setTimeout(resolve, 3_000) })
      await Promise.race([exited, timeout])
      clearTimeout(timer)
      if (child.exitCode === null) child.kill('SIGKILL')
    }
    await this.pending?.catch(() => undefined)
  }

  dispose(): void { void this.stop() }

  private async startProcess(
    bindAddress: string,
    generation: number,
  ): Promise<ManagedMetaHub> {
    this.failure = undefined
    const child = this.spawnProcess(this.binaryPath, [
      '--meta-hub', bindAddress,
      '--meta-hub-parent-pid', String(this.parentPid),
    ])
    this.child = child
    child.stderr.resume()
    try {
      const value = await waitForHandshake(child, this.parentPid)
      if (this.child !== child || this.generation !== generation) {
        throw new Error('Local MetaHub startup was superseded')
      }
      this.ready = { address: value.address, token: value.token }
      this.managedAddress = value.address
      child.once('exit', (code, signal) => {
        if (this.child !== child || this.generation !== generation) return
        this.child = undefined
        this.ready = undefined
        this.failure = `Local MetaHub exited (code=${String(code)}, signal=${String(signal)})`
      })
      return this.ready
    } catch (error) {
      const owns = this.child === child && this.generation === generation
      if (owns) this.child = undefined
      if (child.exitCode === null) child.kill('SIGKILL')
      const message = errorMessage(error)
      if (owns) this.failure = message
      throw new Error(message)
    }
  }
}

async function waitForHandshake(
  child: MetaHubChild,
  parentPid: number,
): Promise<z.infer<typeof HandshakeSchema>> {
  return new Promise((resolve, reject) => {
    const lines = createInterface({ input: child.stdout, crlfDelay: Infinity })
    const timer = setTimeout(() => finish(new Error('Local MetaHub startup timed out')), 10_000)
    const finish = (error?: Error, value?: z.infer<typeof HandshakeSchema>): void => {
      clearTimeout(timer)
      lines.close()
      child.off('error', onError)
      child.off('exit', onExit)
      error ? reject(error) : resolve(value!)
    }
    const onError = (error: Error): void => finish(error)
    const onExit = (): void => finish(new Error('Local MetaHub exited before ready'))
    child.once('error', onError)
    child.once('exit', onExit)
    lines.on('line', (line) => {
      if (!line.startsWith(PREFIX)) return
      try {
        const value = HandshakeSchema.parse(JSON.parse(line.slice(PREFIX.length)))
        if (value.pid !== child.pid || value.parent_pid !== parentPid) {
          throw new Error('Local MetaHub handshake PID mismatch')
        }
        finish(undefined, value)
      } catch (error) { finish(new Error('Invalid local MetaHub handshake', { cause: error })) }
    })
  })
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

function defaultSpawn(binary: string, args: readonly string[]): MetaHubChild {
  return spawn(binary, [...args], {
    shell: false,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
}
