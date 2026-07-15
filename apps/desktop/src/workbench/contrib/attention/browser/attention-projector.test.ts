import {
  permissionItem,
  questionAnswer,
  questionIndex,
  questionItems,
  questionRequestId,
} from './attention-projector'

describe('Attention projector', () => {
  it('projects permission and multi-question requests', () => {
    expect(permissionItem({
      id: 'p', sessionId: 's', runtimeId: 'r', generation: 1,
      agentId: 'a', tool: 'shell', title: 'Run', detail: 'ls',
      risk: 'high', createdAt: '2026-07-11T12:00:00.000Z',
    })).toMatchObject({ id: 'a:p', agentId: 'a', command: 'shell', description: 'ls' })
    const items = questionItems({
      id: 'request', sessionId: 's', runtimeId: 'r', generation: 1,
      agentId: 'a', classifierRunning: false,
      classifierStatus: { kind: 'failed', reason: 'provider timeout' },
      createdAt: '2026-07-11T12:00:00.000Z', questions: [
        { question: 'Continue?', header: 'Decision', allowMultiple: false,
          options: [{ label: 'Yes', description: 'Proceed' }] },
        { question: 'Mode?', allowMultiple: true,
          options: [{ label: 'Fast', description: '' }] },
      ],
    }, [
      { selected: ['0:Yes'], other: '' },
      { selected: ['0:Fast'], other: '' },
    ])
    expect(items.map((item) => item.prompt)).toEqual(['Decision: Continue?', 'Mode?'])
    expect(items.map((item) => item.agentId)).toEqual(['a', 'a'])
    expect(items[0]?.classifier).toEqual({
      kind: 'failed', label: 'Auto-answer unavailable · provider timeout',
    })
    expect(items[1]?.classifier).toBeUndefined()
    expect(items[1]?.choices[0]?.description).toBeUndefined()
    expect(items[1]?.selectedChoiceIds).toEqual(['0:Fast'])
    expect(items[1]?.otherText).toBe('')
    expect(items[1]?.submit).toEqual({ requestId: 'a:request', enabled: true })
    expect(questionRequestId('a:request:1')).toBe('a:request')
    expect(questionIndex('a:request:1')).toBe(1)
    expect(questionAnswer('0:Yes')).toBe('Yes')
    expect(questionItems({
      id: 'auto', sessionId: 's', runtimeId: 'r', generation: 1, agentId: 'a',
      classifierRunning: true, createdAt: '2026-07-11T12:00:00.000Z',
      questions: [{ question: 'Auto?', allowMultiple: false, options: [] }],
    })[0]?.classifier).toEqual({ kind: 'running', label: 'Auto-answering · 0.0s' })
    expect(questionItems({
      id: 'done', sessionId: 's', runtimeId: 'r', generation: 1, agentId: 'a',
      classifierRunning: false, classifierStatus: { kind: 'completed', answers: ['Yes'] },
      createdAt: '2026-07-11T12:00:00.000Z',
      questions: [{ question: 'Done?', allowMultiple: false, options: [] }],
    })[0]?.classifier).toEqual({ kind: 'completed', label: 'Auto-answer ready' })
  })
})
