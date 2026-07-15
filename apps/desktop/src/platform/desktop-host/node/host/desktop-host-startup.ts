import { randomUUID } from 'node:crypto'
import { type HostStatus } from '../../../../shared/contracts'
import { withTimeout } from '../process/desktop-process'
import { type DesktopHostGeneration } from './desktop-host-generation'
import {
  activationFailure,
  cleanupFailure,
  DesktopSessionCreationStateUnknownError,
  isTerminationUncertain,
} from './desktop-host-startup-errors'
import {
  type DesktopHostActivation, type DesktopHostOptions, type DesktopHostSession,
} from './desktop-host-types'
import { JsonRpcClient, type JsonRpcNotification } from '../rpc/jsonrpc-client'

export interface DesktopHostStartupHooks {
  readonly owns: () => boolean
  readonly assertOwned: () => void
  readonly setStatus: (status: HostStatus) => void
  readonly notify: (event: JsonRpcNotification) => void
  readonly fail: () => void
}

interface ActivatedGeneration {
  readonly created: Awaited<DesktopHostGeneration['process']['sessionCreated']>
  readonly session: DesktopHostSession
}

export interface DesktopHostRunHooks extends DesktopHostStartupHooks {
  readonly cleanup: () => Promise<void>
  readonly crash: () => void
}

export async function runGeneration(
  generation: DesktopHostGeneration,
  options: DesktopHostOptions,
  activate: DesktopHostActivation | undefined,
  hooks: DesktopHostRunHooks,
): Promise<DesktopHostSession> {
  const activation = activateGeneration(
    generation, activate, options.startupTimeoutMs ?? 20_000,
  )
  void activation.catch(() => undefined)
  try {
    return await initializeGeneration(generation, options, hooks, activation)
  } catch (error) {
    const settledActivation = withTimeout(
      activation,
      options.startupTimeoutMs ?? 20_000,
      'Loopal Desktop Session activation did not settle after cleanup',
    )
    const [cleanup, activated] = await Promise.allSettled([
      hooks.cleanup(), settledActivation,
    ])
    if (generation.process.didReportSession) await activation.catch(() => undefined)
    hooks.crash()
    let failure = cleanup.status === 'rejected' ? cleanupFailure(error, cleanup.reason) : error
    if (activated.status === 'rejected' && isTerminationUncertain(activated.reason)) {
      failure = activationFailure(failure, activated.reason)
    }
    if (!options.resumeSessionId && generation.process.creationMayHaveCommitted) {
      failure = new DesktopSessionCreationStateUnknownError(failure)
    }
    throw withDiagnostics(failure, generation.process.diagnostics)
  }
}

export async function initializeGeneration(
  generation: DesktopHostGeneration,
  options: DesktopHostOptions,
  hooks: DesktopHostStartupHooks,
  activation: Promise<ActivatedGeneration>,
): Promise<DesktopHostSession> {
  const timeout = options.startupTimeoutMs ?? 20_000
  const alive = await withTimeout(
    generation.process.alive,
    timeout,
    'Loopal Desktop Host did not emit alive in time',
  )
  hooks.assertOwned()
  hooks.setStatus('alive')
  hooks.setStatus('registering')
  const connectRpc = options.connectRpc ?? JsonRpcClient.connect
  const connectionState = { claimed: false, disposed: false, abandoned: false }
  const disposeOnce = (rpc: JsonRpcClient): void => {
    if (connectionState.disposed) return
    connectionState.disposed = true
    rpc.dispose()
  }
  const connecting = connectRpc(alive.addr)
  void connecting.then(
    (lateRpc) => {
      if (!connectionState.claimed && (connectionState.abandoned || !hooks.owns())) {
        disposeOnce(lateRpc)
      }
    },
    () => undefined,
  )
  let rpc: JsonRpcClient
  try {
    rpc = await withTimeout(
      connecting,
      timeout,
      'Loopal Desktop Host did not connect to the Hub in time',
    )
  } catch (error) {
    connectionState.abandoned = true
    throw error
  }
  connectionState.claimed = true
  if (!hooks.owns()) {
    disposeOnce(rpc)
    hooks.assertOwned()
  }
  generation.rpc = rpc
  generation.subscriptions.add(rpc.onNotification(hooks.notify))
  generation.subscriptions.add(rpc.onClose(hooks.fail))
  const registration = await withTimeout(
    rpc.call('hub/register', {
      name: options.clientName ?? `loopal-desktop-${randomUUID()}`,
      token: alive.token,
      role: 'ui_client',
    }),
    timeout,
    'Loopal Desktop Host did not register with the Hub in time',
  )
  if (!isRecord(registration) || registration.ok !== true) {
    throw new Error('Loopal Hub rejected the Desktop UI registration')
  }
  const { created, session } = await withTimeout(
    activation, timeout, 'Loopal Desktop Host did not create or report a Session in time',
  )
  hooks.assertOwned()
  const ready = created.phase === 'ready' ? created : await withTimeout(
    generation.process.ready,
    timeout,
    'Loopal Desktop Host did not emit ready in time',
  )
  if (ready.server_version !== alive.server_version
    || session.serverVersion !== alive.server_version) {
    throw new Error('Loopal Desktop Host changed server version during startup')
  }
  if (ready.session_id !== session.sessionId) {
    throw new Error('Loopal Desktop Host changed Session during startup')
  }
  hooks.setStatus('ready')
  return session
}

export async function activateGeneration(
  generation: DesktopHostGeneration,
  activate: DesktopHostActivation | undefined,
  timeout: number,
): Promise<ActivatedGeneration> {
  const created = await generation.process.sessionCreated
  const session = {
    sessionId: created.session_id,
    serverVersion: created.server_version,
    pid: created.pid,
  }
  generation.session = session
  if (activate) {
    await withTimeout(
      activate(session), timeout, 'Loopal Desktop Session activation did not complete in time',
    )
  }
  return { created, session }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function withDiagnostics(error: unknown, diagnostics: readonly string[]): Error {
  const message = error instanceof Error ? error.message : String(error)
  const suffix = diagnostics.length > 0
    ? `\nHost emitted ${diagnostics.length} diagnostic lines.` : ''
  return new Error(`${message}${suffix}`, { cause: error })
}
