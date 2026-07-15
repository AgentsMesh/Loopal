import { richDetail, richTimestamp } from '../../../../../test/fixtures/workbench/rich-session'
import { appendFakeAgentMessage, fakeProducerAgent } from './fake-message-target'

const entry = {
  id: 'message', role: 'user' as const, text: 'hello', createdAt: richTimestamp,
}

describe('fake message targeting', () => {
  it('distinguishes root and child conversations', () => {
    const detail = richDetail()
    expect(appendFakeAgentMessage(detail, 'main', entry)).toBe(true)
    expect(appendFakeAgentMessage(detail, 'agent-root', entry)).toBe(true)
    expect(appendFakeAgentMessage(detail, 'agent-e2e', entry)).toBe(false)
    expect(detail.agents[1]?.conversation).toContainEqual(entry)
    delete detail.agents[1]!.conversation
    expect(appendFakeAgentMessage(detail, 'agent-e2e', entry)).toBe(false)
    expect(fakeProducerAgent(detail, 'main')).toBe('agent-root')
    expect(fakeProducerAgent(detail, 'agent-e2e')).toBe('agent-e2e')
    detail.agents.length = 0
    expect(fakeProducerAgent(detail, 'main')).toBe('main')
    expect(() => appendFakeAgentMessage(detail, 'missing', entry)).toThrow('not available')
  })
})
