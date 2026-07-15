import {
  sessionDetail, sessionOne, sessionTwo, updatedAt,
} from '../../../../../test/support/workbench/api-stub'
import { openFederationConversation } from './federation-conversation'
import { aggregateFederation } from './federation-model'

describe('Federation conversation navigation', () => {
  it('opens the owner session before selecting its projected Agent', async () => {
    const address = '127.0.0.1:9'
    const target = { sessionId: sessionTwo.id, runtimeId: 'runtime-2', generation: 1 }
    const state = {
      state: 'connected' as const, address, hubName: 'two', hubs: [],
      topology: [{ id: 'two/main', name: 'main', hub: 'two', hubPath: ['two'],
        children: [], lifecycle: 'running' as const }],
      refreshedAt: updatedAt,
    }
    const snapshot = aggregateFederation(
      { state: 'running', address }, { [sessionTwo.id]: target }, [{ target, state }],
    )
    const detail = { ...sessionDetail(sessionTwo), metaHub: state }
    const order: string[] = []
    await openFederationConversation(
      { sessionId: sessionTwo.id, agentId: 'two/main' }, snapshot,
      async (sessionId) => {
        order.push(`open:${sessionId}`)
        expect(sessionId).not.toBe(sessionOne.id)
        return detail
      },
      (agentId) => order.push(`agent:${agentId}`),
      () => order.push('conversation'),
      (sessionId) => order.push(`prepare:${sessionId}`),
    )
    expect(order).toEqual([
      `prepare:${sessionTwo.id}`, `open:${sessionTwo.id}`,
      'agent:agent-session-2', 'conversation',
    ])
  })

  it('resolves a qualified remote Agent in its owner projection', async () => {
    const target = { sessionId: sessionOne.id, runtimeId: 'runtime-1', generation: 1 }
    const state = {
      state: 'connected' as const, address: 'local', hubName: 'one', hubs: [],
      topology: [{ id: 'one/reviewer', name: 'reviewer', hub: 'one', hubPath: ['one'],
        parentId: 'one/main', children: [], lifecycle: 'running' as const }],
      refreshedAt: updatedAt,
    }
    const snapshot = aggregateFederation(
      { state: 'running', address: 'local' }, { [sessionOne.id]: target }, [{ target, state }],
    )
    const detail = { ...sessionDetail(sessionOne), metaHub: state, agents: [{
      id: 'shadow-reviewer', name: 'reviewer', status: 'running' as const,
      qualifiedName: 'one/reviewer',
    }] }
    const selectAgent = vi.fn()
    await openFederationConversation(
      { sessionId: sessionOne.id, agentId: 'one/reviewer' }, snapshot,
      async () => detail, selectAgent, vi.fn(), vi.fn(),
    )
    expect(selectAgent).toHaveBeenCalledWith('shadow-reviewer')
  })

  it('does not navigate a topology node without a projected owner', async () => {
    const snapshot = aggregateFederation({ state: 'running', address: 'local' }, {}, [])
    const openSession = vi.fn()
    await openFederationConversation(
      { sessionId: 'missing', agentId: 'remote/main' }, snapshot,
      openSession, vi.fn(), vi.fn(), vi.fn(),
    )
    expect(openSession).not.toHaveBeenCalled()
  })
})
