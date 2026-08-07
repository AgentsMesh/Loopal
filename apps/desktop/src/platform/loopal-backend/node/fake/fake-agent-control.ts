import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  type AgentControlCommand,
  type AgentControlDisposition,
  type AgentControlInput,
  type AgentControlTarget,
  type DesktopEvent,
  type SessionDetail,
} from '../../../../shared/contracts'
import { type AgentControlOperations } from '../attention/loopal-agent-control'
import { type FakeSessionCatalog } from './fake-session-fixtures'

export function bindFakeAgentControl(
  catalog: FakeSessionCatalog,
  now: () => string,
  fire: (event: DesktopEvent) => void,
): AgentControlOperations {
  const service = new FakeAgentControl(catalog, now, fire)
  return {
    interruptAgent: service.interruptAgent.bind(service),
    controlAgent: service.controlAgent.bind(service),
  }
}

class FakeAgentControl implements AgentControlOperations {
  constructor(
    private readonly catalog: FakeSessionCatalog,
    private readonly now: () => string,
    private readonly fire: (event: DesktopEvent) => void,
  ) {}

  async interruptAgent(
    target: AgentControlTarget,
    token = CancellationToken.None,
  ): Promise<void> {
    const { detail, agent } = this.resolve(target, token)
    agent.status = 'waiting'
    this.settle(detail)
    detail.session = { ...detail.session, status: 'waiting', updatedAt: this.now() }
    this.publish(detail)
  }

  async controlAgent(
    input: AgentControlInput,
    token = CancellationToken.None,
  ): Promise<AgentControlDisposition> {
    const { detail, agent } = this.resolve(input.target, token)
    this.apply(detail, agent, input.command)
    detail.session = { ...detail.session, updatedAt: this.now() }
    this.publish(detail)
    return { status: 'applied' }
  }

  private resolve(target: AgentControlTarget, token: CancellationToken) {
    throwIfCancelled(token)
    const detail = this.catalog.details.get(target.sessionId)
    const runtime = this.catalog.runtimes.get(target.runtimeId)
    if (!detail || !runtime || detail.session.activeRuntimeId !== target.runtimeId
      || runtime.sessionId !== target.sessionId || runtime.generation !== target.generation) {
      throw taggedError('RUNTIME_GONE', `Session runtime is gone: ${target.sessionId}`)
    }
    const agent = detail.agents.find((candidate) => candidate.id === target.agentId)
    if (!agent) throw taggedError('AGENT_GONE', `Agent is not in this runtime: ${target.agentId}`)
    return { detail, agent }
  }

  private apply(
    detail: SessionDetail,
    agent: SessionDetail['agents'][number],
    command: AgentControlCommand,
  ): void {
    if (command.type === 'mode') {
      agent.mode = command.mode
      detail.session = { ...detail.session, mode: command.mode }
    } else if (command.type === 'clear') {
      detail.conversation = []
      this.settle(detail)
    } else if (command.type === 'compact' && detail.view) {
      detail.view = {
        ...detail.view,
        compactBanner: command.instructions
          ? `Summarizing conversation context: ${command.instructions}`
          : 'Summarizing conversation context.',
      }
    } else if (command.type === 'model') {
      agent.model = command.model
      detail.session = { ...detail.session, model: command.model }
    } else if (command.type === 'rewind') {
      detail.conversation = detail.conversation.slice(0, command.turnIndex)
    } else if (command.type === 'thinking') {
      agent.thinkingConfig = thinkingLabel(command.config)
    } else if (command.type === 'permission') agent.permissionMode = command.mode
    else if (command.type === 'decision') agent.decisionMode = command.mode
    else if (command.type === 'sandbox') agent.sandboxPolicy = command.policy
    else if (command.type === 'suspend') {
      agent.status = 'suspended'
      this.settle(detail)
    }
    else if (command.type === 'unsuspend') agent.status = 'waiting'
    else this.applyResource(detail, command)
  }

  private applyResource(detail: SessionDetail, command: AgentControlCommand): void {
    const view = detail.view
    if (!view) return
    if (command.type === 'mcp_reconnect' || command.type === 'mcp_disconnect') {
      view.mcpServers = view.mcpServers.map((server) => server.name === command.server
        ? { ...server, status: command.type === 'mcp_reconnect' ? 'ready' : 'disconnected' }
        : server)
    } else if (command.type === 'background_task_kill') {
      view.backgroundTasks = view.backgroundTasks.map((task) => task.id === command.id
        ? { ...task, status: 'killed' as const }
        : task)
    } else if (command.type === 'cron_delete') {
      view.crons = view.crons.filter((cron) => cron.id !== command.id)
    }
  }

  private publish(detail: SessionDetail): void {
    this.fire({ type: 'session_updated', session: { ...detail.session } })
    this.fire({ type: 'session_detail_replaced', detail: structuredClone(detail) })
  }

  private settle(detail: SessionDetail): void {
    if (detail.view) detail.view = {
      ...detail.view, streamingText: '', streamingThinking: '', thinkingActive: false,
      compactBanner: null,
    }
  }
}

function thinkingLabel(config: Extract<AgentControlCommand, { type: 'thinking' }>['config']): string {
  if (config.type === 'effort') return config.level
  if (config.type === 'budget') return `${config.tokens} tokens`
  return config.type
}

function taggedError(code: string, message: string): Error & { code: string } {
  return Object.assign(new Error(message), { code })
}
