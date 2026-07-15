import {
  type AgentSummary,
  type SessionView,
} from '../../../../shared/contracts'
import { projectMessages } from './loopal-message-projection'
import { type Topology, type ViewSnapshot } from '../runtime/loopal-wire'

export function projectAgent(
  snapshot: ViewSnapshot,
  topology: Topology['agents'][number] | undefined,
  now: Date,
  previous?: AgentSummary,
): AgentSummary {
  const agent = snapshot.state.agent
  const conversation = agent.conversation
  const tools = conversation.messages.flatMap((message) => message.tool_calls)
  const activeTools = tools.filter((tool) => (
    tool.state.state === 'pending' || tool.state.state === 'running'
  ))
  const parent = topology?.parent ?? agent.parent
  return {
    id: agent.name,
    name: agent.name === 'main' ? 'Loopal' : agent.name,
    status: agentStatus(agent.observable.status, topology?.lifecycle),
    ...(parent ? { parentId: leafName(parent) } : {}),
    children: topology?.children ?? agent.children,
    ...(topology?.error ? { error: topology.error } : {}),
    ...(topology?.shadow ? { shadow: true } : {}),
    ...(agent.observable.model ? { model: agent.observable.model } : {}),
    mode: agent.observable.mode,
    thinkingConfig: agent.observable.thinking_config,
    permissionMode: agent.observable.permission_mode,
    decisionMode: agent.observable.decision_mode,
    sandboxPolicy: agent.observable.sandbox_policy,
    ...(activeTools.at(-1)?.summary ? { lastTool: activeTools.at(-1)!.summary } : {}),
    telemetry: {
      turnCount: conversation.turn_count,
      inputTokens: conversation.input_tokens,
      outputTokens: conversation.output_tokens,
      cacheCreationTokens: conversation.cache_creation_tokens,
      cacheReadTokens: conversation.cache_read_tokens,
      thinkingTokens: conversation.thinking_tokens,
      contextWindow: conversation.context_window,
      toolsInFlight: activeTools.length,
      toolCount: tools.length,
    },
    conversation: projectMessages(snapshot, now, previous?.conversation),
    view: projectSessionView(snapshot),
  }
}

export function topologyAgent(
  value: Topology['agents'][number],
  previous?: AgentSummary,
): AgentSummary {
  return {
    id: value.name,
    name: value.name === 'main' ? 'Loopal' : value.name,
    status: topologyStatus(value.lifecycle),
    ...(value.parent ? { parentId: leafName(value.parent) } : {}),
    children: value.children,
    ...(value.error ? { error: value.error } : {}),
    ...(value.shadow ? { shadow: true } : {}),
    ...(value.model ? { model: value.model } : {}),
    ...(previous?.conversation ? { conversation: previous.conversation } : {}),
    ...(previous?.telemetry ? { telemetry: previous.telemetry } : {}),
    ...(previous?.view ? { view: previous.view } : {}),
  }
}

export function retiredAgent(previous: AgentSummary): AgentSummary {
  if (previous.id === 'main' || ['completed', 'failed'].includes(previous.status)) return previous
  const { lastTool: _lastTool, ...rest } = previous
  return { ...rest, status: 'completed' }
}

export function projectSessionView(snapshot: ViewSnapshot): SessionView {
  const state = snapshot.state
  const conversation = state.agent.conversation
  return {
    revision: snapshot.rev,
    historyTruncated: conversation.history_truncated,
    streamingText: conversation.streaming_text,
    streamingThinking: conversation.streaming_thinking,
    thinkingActive: conversation.thinking_active,
    retryBanner: conversation.retry_banner ?? null,
    compactBanner: conversation.compact_banner ?? null,
    tasks: state.tasks.map((task) => ({
      id: task.id, subject: task.subject, description: task.description,
      ...(task.active_form ? { activeForm: task.active_form } : {}),
      status: taskStatus(task.status), blockedBy: task.blocked_by, blocks: task.blocks,
    })),
    backgroundTasks: Object.values(state.bg_tasks).map((task) => ({
      id: task.id, description: task.description, status: bgStatus(task.status),
      exitCode: task.exit_code ?? null, output: task.output,
      createdAt: new Date(task.created_at_unix_ms).toISOString(),
    })),
    crons: state.crons.map((cron) => ({
      id: cron.id, schedule: cron.cron_expr, prompt: cron.prompt,
      recurring: cron.recurring, durable: cron.durable,
      ...(cron.next_fire_unix_ms !== undefined && cron.next_fire_unix_ms !== null
        ? { nextFireAt: new Date(cron.next_fire_unix_ms).toISOString() }
        : {}),
    })),
    mcpServers: (state.mcp_status ?? []).map((server) => ({
      name: server.name, transport: server.transport, source: server.source,
      status: server.status, toolCount: server.tool_count,
      resourceCount: server.resource_count, promptCount: server.prompt_count,
      errors: server.errors,
    })),
    ...(state.thread_goal ? { goal: {
      id: state.thread_goal.goal_id, objective: state.thread_goal.objective,
      status: goalStatus(state.thread_goal.status),
      createdAt: state.thread_goal.created_at, updatedAt: state.thread_goal.updated_at,
    } } : {}),
    ...(state.hub_degraded_since_ms !== undefined && state.hub_degraded_since_ms !== null
      ? { hubDegradedSince: new Date(state.hub_degraded_since_ms).toISOString() }
      : {}),
  }
}

export function normalizeAgentStatus(value: string): AgentSummary['status'] {
  if (value === 'Starting') return 'starting'
  if (value === 'Running') return 'running'
  if (value === 'WaitingForInput') return 'waiting'
  if (value === 'Suspended') return 'suspended'
  if (value === 'Finished') return 'completed'
  if (value === 'Error') return 'failed'
  return 'idle'
}

function agentStatus(
  observable: string,
  lifecycle: Topology['agents'][number]['lifecycle'] | undefined,
): AgentSummary['status'] {
  if (lifecycle === 'spawning') return 'starting'
  if (lifecycle === 'finished') return 'completed'
  if (lifecycle === 'failed') return 'failed'
  return normalizeAgentStatus(observable)
}

function topologyStatus(value: string): AgentSummary['status'] {
  if (value === 'spawning') return 'starting'
  if (value === 'running') return 'running'
  if (value === 'finished') return 'completed'
  if (value === 'failed') return 'failed'
  return 'idle'
}

function taskStatus(value: string): 'pending' | 'in_progress' | 'completed' {
  return value === 'in_progress' || value === 'completed' ? value : 'pending'
}

function bgStatus(value: string): 'running' | 'completed' | 'failed' | 'killed' {
  const lower = value.toLowerCase()
  return lower === 'completed' || lower === 'failed' || lower === 'killed' ? lower : 'running'
}

function goalStatus(value: string): 'active' | 'paused' | 'complete' | 'infeasible' {
  const lower = value.toLowerCase()
  return lower === 'paused' || lower === 'complete' || lower === 'infeasible' ? lower : 'active'
}

function leafName(value: string): string {
  return value.split('/').at(-1) || value
}
