import { type ConversationEntry, type SessionDetail } from '../../../../shared/contracts'

export function appendFakeAgentMessage(
  detail: SessionDetail,
  requestedAgentId: string,
  entry: ConversationEntry,
): boolean {
  const root = detail.agents.find((agent) => !agent.parentId)
  const target = requestedAgentId === 'main'
    ? root
    : detail.agents.find((agent) => agent.id === requestedAgentId)
  if (!target) throw new Error(`Agent is not available: ${requestedAgentId}`)
  if (!target.parentId) return true
  target.conversation = [...target.conversation ?? [], entry]
  return false
}

export function fakeProducerAgent(detail: SessionDetail, requestedAgentId: string): string {
  return requestedAgentId === 'main'
    ? detail.agents.find((agent) => !agent.parentId)?.id ?? 'main'
    : requestedAgentId
}
