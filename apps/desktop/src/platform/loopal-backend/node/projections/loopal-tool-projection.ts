import { type ToolInvocation } from '../../../../shared/contracts'
import { type ViewSnapshot } from '../runtime/loopal-wire'

type WireTool = ViewSnapshot['state']['agent']['conversation']['messages'][number]['tool_calls'][number]

export function projectTool(tool: WireTool): ToolInvocation {
  const state = tool.state
  const status = toolStatus(state)
  const outcome = record(state.outcome)
  const output = status === 'succeeded' && typeof outcome?.content === 'string'
    ? outcome.content
    : undefined
  const failure = status === 'failed' && typeof outcome?.error === 'string'
    ? outcome.error
    : undefined
  const progress = record(state.last_progress)?.tail
  const detail = failure ?? stateDetail(state, tool.metadata)
  return {
    id: tool.id,
    name: tool.name,
    summary: tool.summary || tool.name,
    status,
    ...(tool.input !== undefined ? { input: tool.input } : {}),
    ...(output !== undefined ? { output } : {}),
    ...(typeof progress === 'string' ? { progress } : {}),
    ...(detail !== undefined ? { detail } : {}),
    ...(durationMilliseconds(state.duration) !== undefined
      ? { durationMs: durationMilliseconds(state.duration)! }
      : {}),
    ...(tool.batch_id ? { batchId: tool.batch_id } : {}),
  }
}

function toolStatus(state: Record<string, unknown>): ToolInvocation['status'] {
  if (state.state === 'pending') return 'pending'
  if (state.state === 'running') return 'running'
  if (state.state === 'stale') return 'stale'
  if (state.state === 'cancelled') return 'cancelled'
  if (state.state === 'done') {
    return record(state.outcome)?.type === 'failure' ? 'failed' : 'succeeded'
  }
  return 'failed'
}

function stateDetail(
  state: Record<string, unknown>,
  metadata: unknown,
): string | undefined {
  if (state.state === 'stale' && typeof state.reason === 'string') return state.reason
  if (state.state === 'cancelled' && typeof state.cause === 'string') return state.cause
  const value = record(metadata)
  if (!value || typeof value.kind !== 'string') return undefined
  if (typeof value.reason === 'string') return `${value.kind}: ${value.reason}`
  if (typeof value.cause === 'string') return `${value.kind}: ${value.cause}`
  if (typeof value.count === 'number') return `${value.count} bytes written`
  return value.kind
}

function durationMilliseconds(value: unknown): number | undefined {
  const duration = record(value)
  if (!duration) return undefined
  const secs = typeof duration.secs === 'number' ? duration.secs : 0
  const nanos = typeof duration.nanos === 'number' ? duration.nanos : 0
  return Math.max(0, secs * 1_000 + nanos / 1_000_000)
}

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined
}
