import { type AgentSummary, type ConversationEntry } from '../../../../shared/contracts'

export function normalizeRole(role: string): ConversationEntry['role'] {
  return role === 'user' || role === 'assistant' ? role : 'system'
}

export function normalizeAgentStatus(status: string): AgentSummary['status'] {
  if (status === 'Starting') return 'starting'
  if (status === 'Running') return 'running'
  if (status === 'WaitingForInput') return 'waiting'
  if (status === 'Suspended') return 'suspended'
  if (status === 'Finished') return 'completed'
  if (status === 'Error') return 'failed'
  return 'idle'
}
