import { type DesktopEvent } from '../../../../shared/contracts'
import { LoopalLiveAttention } from './loopal-live-attention'

const now = () => new Date('2026-07-11T12:00:00.000Z')
const scope = {
  workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime', generation: 2,
}

describe('LoopalLiveAttention', () => {
  it('routes each Agent independently and retires only unresolved requests', () => {
    const events: DesktopEvent[] = []
    const attention = new LoopalLiveAttention(scope, now, (event) => events.push(event))
    attention.accept('permission_requested', {
      id: 'shared', name: 'Bash', input: { command: 'pwd' },
    }, 'worker')
    attention.accept('question_requested', {
      id: 'shared', questions: [{ question: 'Continue?', options: [], allow_multiple: false }],
    }, 'main')
    attention.accept('permission_requested', {}, 'worker')
    attention.accept('permission_resolved', { id: 'shared' }, 'worker')
    expect(events).toEqual(expect.arrayContaining([
      expect.objectContaining({
        type: 'permission_requested', request: expect.objectContaining({ agentId: 'worker' }),
      }),
      expect.objectContaining({
        type: 'question_requested', request: expect.objectContaining({ agentId: 'main' }),
      }),
    ]))
    expect(attention.retire()).toEqual([expect.objectContaining({
      type: 'question_resolved', agentId: 'main', requestId: 'shared',
    })])
    expect(attention.retire()).toEqual([])
  })

  it('rehydrates snapshot requests and resolves entries absent from authority', () => {
    const events: DesktopEvent[] = []
    const attention = new LoopalLiveAttention(scope, now, (event) => events.push(event))
    attention.accept('permission_requested', {
      id: 'old', name: 'Write', input: {},
    }, 'main')
    attention.reconcile([{
      kind: 'permission_requested', agentId: 'main',
      value: { id: 'old', name: 'Write', input: {} },
    }])
    events.length = 0
    attention.reconcile([
      {
        kind: 'permission_requested', agentId: 'worker',
        value: { id: 'new', name: 'Read', input: 'README.md' },
      },
      {
        kind: 'question_requested', agentId: 'worker',
        value: {
          id: 'question', classifier_running: true,
          questions: [{ question: 'Pick', options: [], allow_multiple: false }],
        },
      },
      { kind: 'permission_requested', agentId: 'worker', value: {} },
    ])
    expect(events[0]).toMatchObject({
      type: 'permission_resolved', agentId: 'main', requestId: 'old',
    })
    expect(events).toEqual(expect.arrayContaining([
      expect.objectContaining({
        type: 'permission_requested', request: expect.objectContaining({ id: 'new' }),
      }),
      expect.objectContaining({
        type: 'question_requested',
        request: expect.objectContaining({ id: 'question', classifierRunning: true }),
      }),
    ]))
    events.length = 0
    attention.reconcile([])
    expect(events).toHaveLength(2)
    expect(events.map((event) => event.type).sort()).toEqual([
      'permission_resolved', 'question_resolved',
    ])
  })

  it('retains remote requests while their authoritative snapshot is unavailable', () => {
    const events: DesktopEvent[] = []
    const attention = new LoopalLiveAttention(scope, now, (event) => events.push(event))
    attention.accept('question_requested', {
      id: 'remote-question',
      questions: [{ question: 'Remote?', options: [], allow_multiple: false }],
    }, 'hub-b/worker')
    events.length = 0

    expect(attention.remoteAgentIds()).toEqual(new Set(['hub-b/worker']))
    attention.reconcile([])
    expect(events).toEqual([])
    attention.accept('question_resolved', { id: 'remote-question' }, 'hub-b/worker')
    expect(events).toEqual([expect.objectContaining({
      type: 'question_resolved', agentId: 'hub-b/worker', requestId: 'remote-question',
    })])
    expect(attention.remoteAgentIds()).toEqual(new Set())
    expect(attention.retire()).toEqual([])
  })

  it('resolves a remote request absent from a successful authoritative snapshot', () => {
    const events: DesktopEvent[] = []
    const attention = new LoopalLiveAttention(scope, now, (event) => events.push(event))
    attention.accept('question_requested', {
      id: 'remote-question',
      questions: [{ question: 'Remote?', options: [], allow_multiple: false }],
    }, 'hub-b/worker')
    events.length = 0

    attention.reconcile([], new Set(['hub-b/worker']))

    expect(events).toEqual([expect.objectContaining({
      type: 'question_resolved', agentId: 'hub-b/worker', requestId: 'remote-question',
    })])
    expect(attention.remoteAgentIds()).toEqual(new Set())
    expect(attention.retire()).toEqual([])
  })
})
