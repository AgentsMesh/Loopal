import {
  CancellationToken,
  throwIfCancelled,
} from '../../../../base/common/cancellation'
import {
  AgentControlDispositionSchema,
  type AgentControlDisposition,
  type AgentControlInput,
  type AgentControlTarget,
} from '../../../../shared/contracts'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'
import { toHubControlCommand } from '../runtime/loopal-control-wire'
import { MetaHubTopologyWireSchema } from '../federation/loopal-metahub-wire'
import { AgentListSchema } from '../runtime/loopal-wire'

export interface AgentControlOperations {
  interruptAgent(target: AgentControlTarget, token?: CancellationToken): Promise<void>
  controlAgent(
    input: AgentControlInput,
    token?: CancellationToken,
  ): Promise<AgentControlDisposition>
}

interface ControlSession {
  readonly runtime: SessionRuntimeHandle
}

export interface AgentControlRouter {
  session(sessionId: string): ControlSession | undefined
}

export class LoopalAgentControl implements AgentControlOperations {
  constructor(private readonly router: AgentControlRouter) {}

  async interruptAgent(
    target: AgentControlTarget,
    token = CancellationToken.None,
  ): Promise<void> {
    const runtime = await this.resolve(target, token)
    await this.call(runtime, 'hub/interrupt', { target: target.agentId }, token)
  }

  async controlAgent(
    input: AgentControlInput,
    token = CancellationToken.None,
  ): Promise<AgentControlDisposition> {
    const runtime = await this.resolve(input.target, token)
    let acknowledgement: unknown
    try {
      acknowledgement = await this.call(runtime, 'hub/control', {
        target: input.target.agentId,
        command: toHubControlCommand(input.command),
      }, token)
    } catch (error) {
      if (token.isCancellationRequested) throw error
      const message = errorMessage(error)
      if (message.toLowerCase().includes('timed out')) return { status: 'unknown' }
      const legacyRejection = legacyRejectionReason(message)
      if (legacyRejection) return { status: 'rejected', reason: legacyRejection }
      throw controlError('CONTROL_FAILED', `Agent control failed: ${message}`)
    }
    const disposition = AgentControlDispositionSchema.safeParse(acknowledgement)
    if (!disposition.success) {
      throw controlError(
        'CONTROL_PROTOCOL',
        'Agent control returned an invalid disposition',
      )
    }
    return disposition.data
  }

  private async resolve(
    target: AgentControlTarget,
    token: CancellationToken,
  ): Promise<SessionRuntimeHandle> {
    throwIfCancelled(token)
    const session = this.router.session(target.sessionId)
    const runtime = session?.runtime
    if (!runtime || runtime.sessionId !== target.sessionId
      || runtime.runtimeId !== target.runtimeId
      || runtime.generation !== target.generation) {
      throw controlError('RUNTIME_GONE', `Session runtime is gone: ${target.sessionId}`)
    }
    const agents = AgentListSchema.parse(await this.call(runtime, 'hub/list_agents', {}, token))
    const remote = target.agentId.includes('/')
    const localConnected = agents.agents.some((agent) => (
      agent.name === target.agentId && (agent.state === 'connected' || agent.state === 'local')
    ))
    const connected = remote
      ? remoteConnected(MetaHubTopologyWireSchema.parse(
          await this.call(runtime, 'meta/topology', {}, token),
        ), target.agentId)
      : localConnected
    if (!connected) {
      throw controlError('AGENT_GONE', `Agent is not in this runtime: ${target.agentId}`)
    }
    const current = this.router.session(target.sessionId)?.runtime
    if (current !== runtime || current.host.currentStatus !== 'ready') {
      throw controlError('RUNTIME_GONE', `Session runtime is gone: ${target.sessionId}`)
    }
    return runtime
  }

  private async call(
    runtime: SessionRuntimeHandle,
    method: 'hub/control' | 'hub/interrupt' | 'hub/list_agents' | 'meta/topology',
    params: unknown,
    token: CancellationToken,
  ): Promise<unknown> {
    throwIfCancelled(token)
    const controller = new AbortController()
    const subscription = token.onCancellationRequested(() => controller.abort())
    try {
      const result = await runtime.host.request(method, params, controller.signal)
      throwIfCancelled(token)
      return result
    } finally {
      subscription.dispose()
    }
  }
}

function remoteConnected(
  topology: ReturnType<typeof MetaHubTopologyWireSchema.parse>, target: string,
): boolean {
  const separator = target.indexOf('/')
  if (separator <= 0 || separator === target.length - 1) return false
  const hubName = target.slice(0, separator)
  const agentName = target.slice(separator + 1)
  return topology.hubs.some((hub) => hub.hub === hubName
    && 'agents' in hub.topology
    && hub.topology.agents.some((agent) => agent.name === agentName
      && (agent.lifecycle === 'running' || agent.lifecycle === 'spawning')))
}

function controlError(code: string, message: string): Error & { code: string } {
  return Object.assign(new Error(message), { code })
}

function legacyRejectionReason(message: string): string | undefined {
  const marker = 'control rejected:'
  const index = message.toLowerCase().lastIndexOf(marker)
  if (index < 0) return undefined
  return message.slice(index + marker.length).trim() || 'Agent rejected control'
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
