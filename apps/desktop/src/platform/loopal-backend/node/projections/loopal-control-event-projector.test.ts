import { LoopalEventProjector } from './loopal-event-projector'

const now = () => new Date('2026-07-14T12:00:00.000Z')

describe('LoopalEventProjector runtime control notices', () => {
  it('appends structured notices only after authoritative runtime events', () => {
    const append = vi.fn()
    const appendAgent = vi.fn()
    const projector = new LoopalEventProjector(now, {
      append,
      appendAgent,
      updateSession: vi.fn(),
      attention: vi.fn(),
    })
    projector.finishSync(0, { worker: 0 })

    projector.accept(wire({ ModeChanged: { mode: 'plan' } }, 1))
    projector.accept(wire({ Compacted: {
      kept: 4, summarized: 7, tokens_before: 8_000, tokens_after: 3_000,
      strategy: 'manual', files_rehydrated: 0,
    } }, 1, 'worker'))

    expect(append).toHaveBeenCalledWith(expect.objectContaining({
      role: 'system', text: 'Agent mode changed to plan.',
      eventNotice: { kind: 'mode_changed', values: { value: 'plan' } },
    }))
    expect(appendAgent).toHaveBeenCalledWith(expect.objectContaining({
      role: 'system', text: 'Context compacted: 8000 → 3000 tokens.',
      eventNotice: {
        kind: 'context_compacted', values: { tokensBefore: 8_000, tokensAfter: 3_000 },
      },
    }), 'worker')
  })
})

function wire(payload: unknown, rev: number, agent = 'main') {
  return {
    agent_name: { hub: [], agent }, event_id: rev,
    turn_id: 1, correlation_id: 2, rev, payload,
  }
}
