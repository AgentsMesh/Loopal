import { DeferredPromise } from '../../../../base/common/async'
import { Emitter } from '../../../../base/common/event'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { type DesktopHostActivation } from '../../../desktop-host/node/host/desktop-host'
import { SessionRuntimeRegistry } from './session-runtime-registry'

export class FakeRuntimeHost implements DesktopHostClient {
  private readonly statuses = new Emitter<DesktopHostClient['currentStatus']>()
  private readonly notifications = new Emitter<{ method: string; params: unknown }>()
  private readonly gate = new DeferredPromise<void>()
  private readonly stopGate = new DeferredPromise<void>()
  private delayed = false
  private delayedStop = false
  currentStatus: DesktopHostClient['currentStatus'] = 'stopped'
  startCalls = 0
  stopCalls = 0
  disposeCalls = 0
  stopError?: Error
  emitSessionCreated = false
  failAfterSessionCreated?: Error
  activationProvided = false

  readonly onStatus = this.statuses.event
  readonly onNotification = this.notifications.event

  constructor(readonly sessionId: string) {}

  delayStart(): void {
    this.delayed = true
  }

  releaseStart(): void {
    this.gate.resolve(undefined)
  }

  delayStop(): void {
    this.delayedStop = true
  }

  releaseStop(): void {
    this.stopGate.resolve(undefined)
  }

  async start(activate?: DesktopHostActivation) {
    this.startCalls += 1
    this.activationProvided = activate !== undefined
    this.setStatus('spawning')
    if (this.delayed) await this.gate.promise
    const session = { sessionId: this.sessionId, serverVersion: 'test', pid: this.startCalls }
    if (this.emitSessionCreated) {
      await activate?.(session)
      if (this.failAfterSessionCreated) throw this.failAfterSessionCreated
    }
    this.setStatus('ready')
    return session
  }

  async request(method: string, params?: unknown): Promise<unknown> {
    return { method, params }
  }

  async stop(): Promise<void> {
    this.stopCalls += 1
    this.setStatus('stopping')
    if (this.delayedStop) await this.stopGate.promise
    if (this.stopError) throw this.stopError
    this.setStatus('stopped')
  }

  crash(): void {
    this.setStatus('crashed')
  }

  notify(method: string, params: unknown): void {
    this.notifications.fire({ method, params })
  }

  dispose(): void {
    this.disposeCalls += 1
    this.statuses.dispose()
    this.notifications.dispose()
  }

  private setStatus(status: DesktopHostClient['currentStatus']): void {
    this.currentStatus = status
    this.statuses.fire(status)
  }
}

export function createRegistryHarness(options: {
  maxLive?: number
  maxTombstones?: number
  freshSessions?: string[]
} = {}) {
  const hosts: FakeRuntimeHost[] = []
  const inputs: Array<{ cwd: string; resumeSessionId?: string }> = []
  const runtimeIds: string[] = []
  const fresh = [...(options.freshSessions ?? ['fresh-a', 'fresh-b', 'fresh-c'])]
  let delayedNext = false
  let nextRuntime = 0
  const registry = new SessionRuntimeRegistry({
    maxLive: options.maxLive ?? 4,
    ...(options.maxTombstones === undefined ? {} : { maxTombstones: options.maxTombstones }),
    createRuntimeId: () => {
      const id = `runtime-${++nextRuntime}`
      runtimeIds.push(id)
      return id
    },
    createHost: (input) => {
      inputs.push(input)
      const host = new FakeRuntimeHost(input.resumeSessionId ?? fresh.shift() ?? 'fresh')
      if (delayedNext) {
        delayedNext = false
        host.delayStart()
      }
      hosts.push(host)
      return host
    },
  })
  return {
    registry,
    hosts,
    inputs,
    runtimeIds,
    delayNext: () => { delayedNext = true },
  }
}

export const workspaceA = { workspaceId: 'workspace-a', cwd: '/workspace/a' }
export const workspaceB = { workspaceId: 'workspace-b', cwd: '/workspace/b' }
