import { createHash } from 'node:crypto'
import { type ConversationEntry } from '../../../../shared/contracts'
import { projectTool } from './loopal-tool-projection'
import { type ViewSnapshot } from '../runtime/loopal-wire'

type WireMessage = ViewSnapshot['state']['agent']['conversation']['messages'][number]

export function projectMessages(
  snapshot: ViewSnapshot,
  now: Date,
  previous: readonly ConversationEntry[] = [],
): ConversationEntry[] {
  const agent = snapshot.state.agent
  const old = new Map(previous.map((entry) => [entry.id, entry]))
  const occurrences = new Map<string, number>()
  const entries = agent.conversation.messages.map((message) => {
    const id = message.message_id ?? fallbackId(agent.name, message, occurrences)
    return projectMessage(message, agent.name, id, old.get(id)?.createdAt ?? now.toISOString())
  })
  appendStreaming(entries, old, agent.name, 'thinking', agent.conversation.streaming_thinking, now)
  appendStreaming(entries, old, agent.name, 'assistant', agent.conversation.streaming_text, now)
  return mergeEventNotices(entries, previous, agent.name)
}

function mergeEventNotices(
  entries: ConversationEntry[],
  previous: readonly ConversationEntry[],
  agentId: string,
): ConversationEntry[] {
  const ids = new Set(entries.map((entry) => entry.id))
  const notices = previous.filter((entry) => (
    entry.eventNotice && (entry.agentId ?? agentId) === agentId && !ids.has(entry.id)
  )).slice(-64)
  return [...entries, ...notices].sort((left, right) => (
    left.createdAt.localeCompare(right.createdAt)
  ))
}

function projectMessage(
  message: WireMessage,
  agentId: string,
  id: string,
  createdAt: string,
): ConversationEntry {
  const skill = message.skill_info
  const inbox = message.inbox
  const role = normalizeConversationRole(message.role)
  const thinking = role === 'thinking' ? parseThinking(message.content) : undefined
  return {
    id,
    role,
    text: thinking?.text ?? message.content,
    createdAt,
    agentId,
    imageCount: message.image_count,
    toolCalls: message.tool_calls.map(projectTool),
    ...(thinking ? { thinkingTokens: thinking.tokens } : {}),
    ...(skill ? { skill: { name: skill.name, userArgs: skill.user_args } } : {}),
    ...(inbox ? {
      inbox: {
        source: stringifySource(inbox.source),
        ...(inbox.summary ? { summary: inbox.summary } : {}),
      },
    } : {}),
  }
}

function appendStreaming(
  entries: ConversationEntry[],
  previous: ReadonlyMap<string, ConversationEntry>,
  agentId: string,
  role: 'thinking' | 'assistant',
  text: string,
  now: Date,
): void {
  if (!text) return
  const id = `${agentId}-streaming-${role}`
  entries.push({
    id, role, text, agentId, streaming: true,
    createdAt: previous.get(id)?.createdAt ?? now.toISOString(),
  })
}

export function normalizeConversationRole(value: string): ConversationEntry['role'] {
  if (value === 'user' || value === 'assistant' || value === 'thinking'
    || value === 'error' || value === 'welcome') return value
  return 'system'
}

function fallbackId(
  agentId: string,
  message: WireMessage,
  occurrences: Map<string, number>,
): string {
  const digest = createHash('sha256').update(JSON.stringify([
    message.role, message.content, message.image_count,
    message.tool_calls.map((tool) => tool.id),
  ])).digest('hex').slice(0, 16)
  const count = occurrences.get(digest) ?? 0
  occurrences.set(digest, count + 1)
  return `${agentId}-message-${digest}${count ? `-${count}` : ''}`
}

function parseThinking(content: string): { tokens: number; text: string } {
  const [first, ...rest] = content.split('\n')
  const tokens = /^\d+$/.test(first ?? '') ? Number(first) : 0
  return tokens > 0 ? { tokens, text: rest.join('\n') } : { tokens: 0, text: content }
}

function stringifySource(value: unknown): string {
  if (typeof value === 'string') {
    return value === 'Human' || value === 'Scheduled' ? value.toLowerCase() : value
  }
  if (isRecord(value)) {
    const address = sourceAddress(value)
    if (address) return address
    if (typeof value.System === 'string') return `system:${value.System}`
  }
  try { return JSON.stringify(value) }
  catch { return 'agent' }
}

function sourceAddress(value: Record<string, unknown>): string | undefined {
  if (isRecord(value.Agent)) return formatAddress(value.Agent)
  if (isRecord(value.AgentResult) && isRecord(value.AgentResult.child)) {
    return formatAddress(value.AgentResult.child)
  }
  if (isRecord(value.Channel) && isRecord(value.Channel.from)) {
    return formatAddress(value.Channel.from)
  }
  return undefined
}

function formatAddress(value: Record<string, unknown>): string | undefined {
  if (typeof value.agent !== 'string') return undefined
  const hubs = Array.isArray(value.hub)
    ? value.hub.filter((part): part is string => typeof part === 'string') : []
  return [...hubs, value.agent].join('/')
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
