import {
  AttentionDesktopEventSchema,
  PermissionRequestSchema,
  PermissionResponseInputSchema,
  QuestionRequestSchema,
  QuestionResponseInputSchema,
} from './attention-contracts'

const intentDigest = `sha256:${'ab'.repeat(32)}`

describe('attention contracts', () => {
  it('validates permission and multi-question lifecycles', () => {
    const permission = PermissionRequestSchema.parse({
      id: 'p', sessionId: 'session', runtimeId: 'runtime', generation: 1,
      agentId: 'main', tool: 'bash', title: 'Allow bash', intentDigest,
      detail: '{}', risk: 'high', createdAt: '2026-07-11T12:00:00.000Z',
    })
    expect(permission.risk).toBe('high')
    expect(PermissionResponseInputSchema.parse({
      sessionId: 'session', runtimeId: 'runtime', generation: 1,
      agentId: 'main', requestId: 'p', intentDigest, decision: 'allow_session',
    }).decision).toBe('allow_session')
    expect(() => PermissionResponseInputSchema.parse({
      sessionId: 'session', runtimeId: 'runtime', generation: 1,
      agentId: 'main', requestId: 'p', decision: 'allow_once',
    })).toThrow('Permission intent digest is required')
    const question = QuestionRequestSchema.parse({
      id: 'q', sessionId: 'session', runtimeId: 'runtime', generation: 1,
      agentId: 'main', classifierRunning: true,
      createdAt: '2026-07-11T12:00:00.000Z',
      questions: [{
        question: 'Continue?', header: 'Decision', allowMultiple: false,
        options: [{ label: 'Yes', description: 'Continue work' }],
      }],
    })
    expect(question.questions[0]?.options[0]?.label).toBe('Yes')
    expect(QuestionResponseInputSchema.parse({
      sessionId: 'session', runtimeId: 'runtime', generation: 1,
      agentId: 'main', requestId: 'q', answers: ['Yes'],
    }).answers).toEqual(['Yes'])
    expect(QuestionResponseInputSchema.parse({
      sessionId: 'session', runtimeId: 'runtime', generation: 1,
      agentId: 'main', requestId: 'q', cancelled: true,
    }).cancelled).toBe(true)
    expect(() => QuestionResponseInputSchema.parse({
      sessionId: 'session', runtimeId: 'runtime', generation: 1,
      agentId: 'main', requestId: 'q', answers: ['Yes'], cancelled: true,
    })).toThrow('Provide answers or cancel')
    expect(AttentionDesktopEventSchema.parse({
      type: 'permission_requested', request: permission,
    }).type).toBe('permission_requested')
    expect(AttentionDesktopEventSchema.parse({
      type: 'permission_resolved', sessionId: 'session', runtimeId: 'runtime',
      generation: 1, agentId: 'main', requestId: 'p',
    }).type).toBe('permission_resolved')
    expect(AttentionDesktopEventSchema.parse({
      type: 'question_requested', request: question,
    }).type).toBe('question_requested')
    expect(AttentionDesktopEventSchema.parse({
      type: 'question_resolved', sessionId: 'session', runtimeId: 'runtime',
      generation: 1, agentId: 'main', requestId: 'q',
    }).type).toBe('question_resolved')
  })
})
