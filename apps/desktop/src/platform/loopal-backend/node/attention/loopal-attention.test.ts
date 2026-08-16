import { projectAttentionEvent } from './loopal-attention'

const now = () => new Date('2026-07-11T12:00:00.000Z')
const scope = {
  workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime', generation: 3,
}
const intentDigest = `sha256:${'ab'.repeat(32)}`

describe('Loopal attention projection', () => {
  it('projects permission requests with risk and readable input', () => {
    const permission = projectAttentionEvent('permission_requested', {
      id: 'p1', name: 'Bash', input: { command: 'rm file' },
      permission_intent: { intent_digest: intentDigest },
    }, scope, 'worker', now)
    expect(permission).toMatchObject({
      type: 'permission_requested',
      request: {
        id: 'p1', agentId: 'worker', tool: 'Bash', risk: 'high', intentDigest,
      },
    })
    expect(permission?.type === 'permission_requested' && permission.request.detail)
      .toContain('"command": "rm file"')
    const legacy = projectAttentionEvent('permission_requested', {
      id: 'legacy', name: 'Write', input: {},
    }, scope, 'worker', now)
    expect(legacy?.type === 'permission_requested' && legacy.request)
      .not.toHaveProperty('intentDigest')
    expect(projectAttentionEvent('permission_requested', {
      id: 'p2', name: 'Read', input: 'README.md',
    }, scope, 'worker', now)).toMatchObject({ request: { risk: 'low', detail: 'README.md' } })
    expect(projectAttentionEvent('permission_requested', {
      id: 'p3', name: 'Unknown', input: 42,
    }, scope, 'worker', now)).toMatchObject({ request: { risk: 'medium', detail: '42' } })
    expect(projectAttentionEvent('permission_requested', {
      id: 'p4', input: null,
    }, scope, 'worker', now)).toMatchObject({ request: { tool: 'tool', risk: 'medium' } })
  })

  it('projects structured multi-question requests and resolution events', () => {
    expect(projectAttentionEvent('question_requested', {
      id: 'q1', classifier_running: true,
      classifier_status: { kind: 'running', elapsed_ms: 1_500 },
      questions: [{
        question: 'Pick one', header: 'Choice', allow_multiple: false,
        options: [
          { label: 'A', description: 'First' },
          { label: '', description: 'ignored' },
          {},
        ],
      }],
    }, scope, 'worker', now)).toMatchObject({
      type: 'question_requested',
      request: {
        id: 'q1', classifierRunning: true,
        classifierStatus: { kind: 'running', elapsedMs: 1_500 },
        questions: [{ question: 'Pick one', options: [{ label: 'A' }] }],
      },
    })
    expect(projectAttentionEvent('permission_resolved', { id: 'p1' }, scope, 'worker', now))
      .toEqual({ type: 'permission_resolved', ...runtimeScope(), agentId: 'worker', requestId: 'p1' })
    expect(projectAttentionEvent('question_resolved', { id: 'q1' }, scope, 'worker', now))
      .toEqual({ type: 'question_resolved', ...runtimeScope(), agentId: 'worker', requestId: 'q1' })
    expect(projectAttentionEvent('question_requested', {
      id: 'q2', classifier_status: { kind: 'failed', reason: 'provider timeout' },
      questions: [{ question: 'Answer?', allow_multiple: false, options: [{ label: 'A' }] }],
    }, scope, 'worker', now)).toMatchObject({
      request: { classifierStatus: { kind: 'failed', reason: 'provider timeout' } },
    })
    expect(projectAttentionEvent('question_requested', {
      id: 'q2-empty', classifier_status: { kind: 'failed' },
      questions: [{ question: 'Answer?', allow_multiple: false, options: [{ label: 'A' }] }],
    }, scope, 'worker', now)).toMatchObject({
      request: { classifierStatus: { kind: 'failed', reason: '' } },
    })
    expect(projectAttentionEvent('question_requested', {
      id: 'q3', classifier_status: { kind: 'completed', answers: ['A', 2] },
      questions: [{ question: 'Answer?', allow_multiple: false, options: [{ label: 'A' }] }],
    }, scope, 'worker', now)).toMatchObject({
      request: { classifierStatus: { kind: 'completed', answers: ['A', '2'] } },
    })
    expect(projectAttentionEvent('question_requested', {
      id: 'q4', classifier_status: { kind: 'completed', answers: null },
      questions: [{ question: 'Answer?', allow_multiple: false, options: [{ label: 'A' }] }],
    }, scope, 'worker', now)).toMatchObject({
      request: { classifierStatus: { kind: 'completed', answers: [] } },
    })
    expect(projectAttentionEvent('question_requested', {
      id: 'q5', classifier_status: { kind: 'none' },
      questions: [{ question: 'Answer?', allow_multiple: false, options: [{ label: 'A' }] }],
    }, scope, 'worker', now)).toMatchObject({
      request: { classifierStatus: { kind: 'none' } },
    })
    for (const elapsed_ms of [-10, Number.NaN, 'invalid']) {
      expect(projectAttentionEvent('question_requested', {
        id: `running-${String(elapsed_ms)}`,
        classifier_status: { kind: 'running', elapsed_ms },
        questions: [{ question: 'Answer?', allow_multiple: false, options: [{ label: 'A' }] }],
      }, scope, 'worker', now)).toMatchObject({
        request: { classifierStatus: { kind: 'running', elapsedMs: 0 } },
      })
    }
  })

  it('drops malformed attention events', () => {
    expect(projectAttentionEvent('permission_requested', null, scope, 'worker', now)).toBeUndefined()
    expect(projectAttentionEvent('permission_requested', {}, scope, 'worker', now)).toBeUndefined()
    expect(projectAttentionEvent('question_requested', {
      id: 'q', questions: 'invalid',
    }, scope, 'worker', now)).toBeUndefined()
    expect(projectAttentionEvent('question_requested', {
      id: 'q', questions: [{ question: 3, options: 'invalid' }],
    }, scope, 'worker', now)).toMatchObject({ type: 'question_requested' })
  })

  it('falls back when permission input cannot be serialized', () => {
    const circular: Record<string, unknown> = {}
    circular.self = circular
    expect(projectAttentionEvent('permission_requested', {
      id: 'p', name: 'tool', input: circular,
    }, scope, 'worker', now)).toMatchObject({ request: { detail: '[object Object]' } })
  })
})

function runtimeScope() {
  return { sessionId: scope.sessionId, runtimeId: scope.runtimeId, generation: scope.generation }
}
