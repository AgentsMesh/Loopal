import { type SessionDetail } from '../../../../shared/contracts'
import {
  type FederationConversationTarget, type FederationSnapshot,
  type FederationTopologyNode,
} from './federation-model'

export async function openFederationConversation(
  target: FederationConversationTarget,
  snapshot: FederationSnapshot,
  openSession: (sessionId: string) => Promise<SessionDetail | undefined>,
  selectAgent: (agentId: string) => void,
  showConversation: () => void,
  prepareSession: (sessionId: string) => void,
): Promise<void> {
  const node = snapshot.topology.find((candidate) =>
    candidate.sessionId === target.sessionId && candidate.agent.id === target.agentId)
  if (!node) return
  prepareSession(target.sessionId)
  const detail = await openSession(target.sessionId)
  if (!detail || detail.session.id !== target.sessionId) return
  const agentId = projectedAgentId(detail, node)
  if (!agentId) return
  selectAgent(agentId)
  showConversation()
}

export function projectedAgentId(
  detail: SessionDetail,
  node: FederationTopologyNode,
): string | undefined {
  const exact = detail.agents.find((agent) =>
    agent.id === node.agent.id || agent.qualifiedName === node.agent.id)
  if (exact) return exact.id
  if (node.agent.hub !== detail.metaHub?.hubName || node.agent.parentId) return undefined
  return detail.agents.find((agent) => !agent.parentId && !agent.qualifiedName)?.id
}
