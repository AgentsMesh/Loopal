import { Emitter, type Event } from '../../../../base/common/event'
import { Disposable } from '../../../../base/common/lifecycle'
import { type HostStatus } from '../../../../shared/contracts'
import { DesktopProcess, spawnDesktopProcess, validateResumeSessionId } from '../process/desktop-process'
import {
  createGeneration,
  terminateGeneration,
  type DesktopHostGeneration,
} from './desktop-host-generation'
import {
  type DesktopHostActivation, type DesktopHostOptions, type DesktopHostSession,
} from './desktop-host-types'
import { createOperation, type DesktopHostOperation } from './desktop-host-operation'
import { runGeneration } from './desktop-host-startup'
import { type JsonRpcNotification } from '../rpc/jsonrpc-client'
import { DESKTOP_HUB_METHODS } from './desktop-host-methods'

export { spawnDesktopProcess } from '../process/desktop-process'
export type {
  DesktopHostActivation, DesktopHostOptions, DesktopHostSession, MetaHubStartupOptions,
} from './desktop-host-types'
export class LoopalDesktopHost extends Disposable {
  private readonly statusEmitter = this.register(new Emitter<HostStatus>())
  private readonly notificationEmitter = this.register(new Emitter<JsonRpcNotification>())
  private status: HostStatus = 'stopped'
  private active: DesktopHostGeneration | undefined
  private starting: DesktopHostOperation<DesktopHostSession> | undefined
  private stopping: DesktopHostOperation<void> | undefined
  private cleanup: Promise<void> | undefined
  private command = 0
  private lastDiagnostics: readonly string[] = []
  readonly onStatus: Event<HostStatus> = this.statusEmitter.event
  readonly onNotification: Event<JsonRpcNotification> = this.notificationEmitter.event
  constructor(private readonly options: DesktopHostOptions) {
    super()
    validateResumeSessionId(options.resumeSessionId)
  }

  get currentStatus(): HostStatus {
    return this.status
  }
  get diagnostics(): readonly string[] {
    return this.active?.process.diagnostics ?? this.lastDiagnostics
  }

  start(activate?: DesktopHostActivation): Promise<DesktopHostSession> {
    const session = this.active?.session
    if (session && this.status === 'ready') {
      return activate ? activate(session).then(() => session) : Promise.resolve(session)
    }
    if (this.starting?.command === this.command) return this.starting.promise
    const command = ++this.command
    const pending = createOperation<DesktopHostSession>(command)
    this.starting = pending.operation
    pending.complete(this.startInternal(command, activate, this.stopping?.promise))
    void pending.operation.promise.then(
      () => this.clearStarting(pending.operation),
      () => this.clearStarting(pending.operation),
    )
    return pending.operation.promise
  }

  request(method: string, params: unknown = {}, signal?: AbortSignal): Promise<unknown> {
    const rpc = this.active?.rpc
    if (!rpc || this.status !== 'ready') {
      return Promise.reject(new Error('Loopal Desktop Host is not ready'))
    }
    if (!DESKTOP_HUB_METHODS.has(method)) {
      return Promise.reject(new Error(`Loopal Desktop Host method is not allowlisted: ${method}`))
    }
    return rpc.call(method, params, signal)
  }

  stop(): Promise<void> {
    if (this.stopping?.command === this.command) return this.stopping.promise
    const command = ++this.command
    const pending = createOperation<void>(command)
    this.stopping = pending.operation
    pending.complete(this.stopInternal(command))
    void pending.operation.promise.then(
      () => this.clearStopping(pending.operation),
      () => this.clearStopping(pending.operation),
    )
    return pending.operation.promise
  }

  override dispose(): void {
    void this.stop()
    super.dispose()
  }

  private async startInternal(
    command: number, activate?: DesktopHostActivation, blocked?: Promise<void>,
  ): Promise<DesktopHostSession> {
    if (blocked) await blocked
    if (this.cleanup) await this.cleanup
    if (this.active) await this.release(this.active, false)
    this.assertCommand(command)
    this.setStatus('spawning')
    const generation = this.spawnGeneration(command)
    return runGeneration(generation, this.options, activate, {
      owns: () => this.owns(generation),
      assertOwned: () => this.assertOwned(generation),
      setStatus: (status) => this.setStatus(status),
      notify: (event) => this.notificationEmitter.fire(event),
      fail: () => this.failGeneration(generation),
      cleanup: () => this.release(generation, false),
      crash: () => {
        if (this.command === command) this.setStatus('crashed')
      },
    })
  }

  private spawnGeneration(command: number): DesktopHostGeneration {
    const parentPid = this.options.parentPid ?? globalThis.process.pid
    const metaHub = this.options.metaHub
    const process = new DesktopProcess({
      binaryPath: this.options.binaryPath,
      cwd: this.options.cwd,
      parentPid,
      env: this.options.env,
      resumeSessionId: this.options.resumeSessionId,
      spawnProcess: this.options.spawnProcess ?? spawnDesktopProcess,
      ...(metaHub ? { metaHub } : {}),
    })
    const generation = createGeneration(command, process)
    this.active = generation
    generation.subscriptions.add(process.onExit((exit) => {
      generation.exited = true
      this.failGeneration(generation, exit.error)
    }))
    return generation
  }

  private failGeneration(generation: DesktopHostGeneration, error?: Error): void {
    if (!this.owns(generation)) return
    delete generation.session
    this.lastDiagnostics = generation.process.diagnostics
    if (this.command === generation.command) this.command += 1
    const cleanup = this.release(generation, false)
    this.setStatus('crashed')
    void cleanup
  }

  private async stopInternal(command: number): Promise<void> {
    const generation = this.active
    if (generation) {
      this.setStatus('stopping')
      await this.release(generation, true)
    } else {
      await this.cleanup
    }
    if (this.command === command) this.setStatus('stopped')
  }

  private release(generation: DesktopHostGeneration, graceful: boolean): Promise<void> {
    const cleanup = terminateGeneration(
      generation,
      graceful,
      this.options.shutdownTimeoutMs ?? 4_000,
    )
    this.cleanup = cleanup
    const finish = (failed: boolean): void => {
      this.lastDiagnostics = generation.process.diagnostics
      if (!failed || generation.exited) {
        if (this.active === generation) this.active = undefined
      } else {
        delete generation.cleanup
      }
      if (this.cleanup === cleanup) this.cleanup = undefined
    }
    void cleanup.then(() => finish(false), () => finish(true))
    return cleanup
  }

  private owns(generation: DesktopHostGeneration): boolean {
    return this.active === generation && !generation.closing
  }
  private assertOwned(generation: DesktopHostGeneration): void {
    if (!this.owns(generation) || this.command !== generation.command) {
      throw new Error('Loopal Desktop Host startup was superseded')
    }
  }

  private assertCommand(command: number): void {
    if (this.command !== command) throw new Error('Loopal Desktop Host startup was superseded')
  }
  private clearStarting(operation: DesktopHostOperation<DesktopHostSession>): void {
    if (this.starting === operation) this.starting = undefined
  }
  private clearStopping(operation: DesktopHostOperation<void>): void {
    if (this.stopping === operation) this.stopping = undefined
  }

  private setStatus(status: HostStatus): void {
    if (status === this.status) return
    this.status = status
    this.statusEmitter.fire(status)
  }
}
