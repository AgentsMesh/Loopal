import { projectEventNotice } from './loopal-event-notice'

describe('projectEventNotice', () => {
  it('projects transient lifecycle events that are absent from ViewSnapshot', () => {
    expect(text('ContinuationSkipped', { reason: 'goal changed' }))
      .toBe('Continuation skipped: goal changed')
    expect(text('DegenerationDetected', {
      signal: 'barren_streak', count: 3,
    })).toBe('Degeneration detected: barren streak (3).')
    expect(text('ContinuationGateChanged', {
      open: false, closed_reason: 'idle_timeout', wake_deadline: '2026-07-12T12:00:00Z',
    })).toBe('Automatic continuation paused: idle timeout until 2026-07-12T12:00:00Z.')
    expect(text('ContinuationGateChanged', { open: true }))
      .toBe('Automatic continuation resumed.')
  })

  it('projects authoritative runtime changes with structured localization data', () => {
    const changes = [
      ['ModeChanged', { mode: 'plan' }, 'Agent mode changed to plan.', 'mode_changed'],
      ['ModelChanged', { model: 'claude-opus' }, 'Model changed to claude-opus.', 'model_changed'],
      ['PermissionModeChanged', { mode: 'ask_dangerous' },
        'Permission mode changed to ask dangerous.', 'permission_mode_changed'],
      ['DecisionModeChanged', { mode: 'manual' },
        'Decision mode changed to manual.', 'decision_mode_changed'],
      ['SandboxPolicyChanged', { policy: 'read_only' },
        'Sandbox policy changed to read only.', 'sandbox_policy_changed'],
    ] as const
    for (const [kind, value, fallback, noticeKind] of changes) {
      expect(projectEventNotice(kind, value)).toEqual({
        text: fallback,
        runtime: { kind: noticeKind, values: { value: Object.values(value)[0] } },
      })
    }
    expect(projectEventNotice('ThinkingChanged', { thinking_config: '{}' })).toEqual({
      text: 'Thinking configuration changed.', runtime: { kind: 'thinking_changed' },
    })
    expect(projectEventNotice('Cleared', { context_window: 200_000 })).toEqual({
      text: 'Conversation cleared.', runtime: { kind: 'conversation_cleared' },
    })
    expect(projectEventNotice('Rewound', { remaining_turns: 2 })).toEqual({
      text: 'Conversation rewound; 2 turns remain.',
      runtime: { kind: 'conversation_rewound', values: { remaining: 2 } },
    })
    expect(projectEventNotice('Compacted', {
      tokens_before: 8_000, tokens_after: 3_000, kept: 4, summarized: 7,
    })).toEqual({
      text: 'Context compacted: 8000 → 3000 tokens.',
      runtime: {
        kind: 'context_compacted', values: { tokensBefore: 8_000, tokensAfter: 3_000 },
      },
    })
  })

  it('ignores malformed warnings and unrelated events', () => {
    expect(projectEventNotice('SessionResumeWarnings', { warnings: [1, null] })).toBeUndefined()
    expect(text('TurnCancelled', null)).toBe('Turn cancelled.')
    expect(text('DegenerationDetected', { count: Number.NaN }))
      .toBe('Degeneration detected.')
    expect(projectEventNotice('Stream', { text: 'answer' })).toBeUndefined()
    expect(projectEventNotice('ModeChanged', {})).toBeUndefined()
  })
})

function text(kind: string, value: unknown): string | undefined {
  return projectEventNotice(kind, value)?.text
}
