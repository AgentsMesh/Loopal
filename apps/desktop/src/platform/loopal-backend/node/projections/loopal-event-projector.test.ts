import { describe, expect, it, vi } from 'vitest'
import { LoopalEventProjector } from './loopal-event-projector'

const now = () => new Date('2026-07-11T12:00:00.000Z')

function wire(payload: unknown, revision?: number, address: unknown = { hub: [], agent: 'main' }) {
  return {
    agent_name: address,
    event_id: 7,
    turn_id: 1,
    correlation_id: 2,
    ...(revision === undefined ? {} : { rev: revision }),
    payload,
  }
}

function harness() {
  const append = vi.fn()
  const appendAgent = vi.fn()
  const updateSession = vi.fn()
  const updateAgentLifecycle = vi.fn()
  const attention = vi.fn()
  const overflow = vi.fn()
  const artifacts = vi.fn()
  const projector = new LoopalEventProjector(now, {
    append, appendAgent, updateSession, updateAgentLifecycle, attention, overflow, artifacts,
  })
  return {
    projector, append, appendAgent, updateSession, updateAgentLifecycle,
    attention, overflow, artifacts,
  }
}

describe('LoopalEventProjector', () => {
  it('buffers during sync, replays fresh revisions, and drops duplicates', () => {
    const { projector, updateSession } = harness()
    projector.accept(wire('Started', 2))
    expect(updateSession).not.toHaveBeenCalled()
    projector.finishSync(1)
    expect(updateSession).toHaveBeenCalledWith('running')
    projector.accept(wire('Running', 2))
    expect(updateSession).toHaveBeenCalledOnce()
    projector.beginSync()
    projector.accept(wire('Running', 4))
    projector.finishSync(3)
    expect(updateSession).toHaveBeenCalledTimes(2)
  })

  it('projects every lifecycle, attention, error, and tool payload', () => {
    const { projector, append, updateSession, updateAgentLifecycle, attention } = harness()
    projector.finishSync(0)
    projector.accept(wire({ Stream: { text: 'answer' } }, 1))
    projector.accept(wire({ ToolPermissionRequest: { id: 'p' } }, 2))
    expect(append).toHaveBeenCalledWith(expect.objectContaining({ role: 'assistant', text: 'answer' }))
    expect(attention).toHaveBeenCalledWith('permission_requested', { id: 'p' }, 'main')
    expect(updateSession).toHaveBeenCalledWith('waiting', 'permission')

    projector.accept(wire({ UserQuestionRequest: { id: 'q' } }, 3))
    projector.accept(wire({ ToolPermissionResolved: { id: 'p' } }, 4))
    projector.accept(wire({ UserQuestionResolved: { id: 'q' } }, 5))
    expect(attention).toHaveBeenCalledWith('question_requested', { id: 'q' }, 'main')
    expect(attention).toHaveBeenCalledWith('permission_resolved', { id: 'p' }, 'main')
    expect(attention).toHaveBeenCalledWith('question_resolved', { id: 'q' }, 'main')

    projector.accept(wire({ Error: { message: 'failed' } }, 6))
    projector.accept(wire({ Error: null }, 7))
    expect(append).toHaveBeenCalledWith(expect.objectContaining({
      id: 'event-7', role: 'error', text: 'failed',
    }))
    expect(append).toHaveBeenCalledWith(expect.objectContaining({ text: 'Loopal runtime failed' }))
    expect(updateSession).toHaveBeenCalledWith('failed', 'failure')

    for (const payload of ['AwaitingInput', 'Finished', { TurnCompleted: {} }]) {
      projector.accept(wire(payload))
    }
    expect(updateSession).not.toHaveBeenCalledWith('waiting', 'completed')
    projector.accept(wire('Running'))
    projector.accept(wire({ TurnCompleted: {} }))
    expect(updateSession).not.toHaveBeenLastCalledWith('waiting', 'completed')
    projector.accept(wire('AwaitingInput'))
    expect(updateSession).toHaveBeenLastCalledWith('waiting', 'completed')
    expect(updateAgentLifecycle).toHaveBeenCalledWith('main', 'Error', { message: 'failed' })
    expect(updateAgentLifecycle).toHaveBeenCalledWith('main', 'Running', undefined)
    expect(updateAgentLifecycle).toHaveBeenCalledWith('main', 'AwaitingInput', undefined)
    projector.accept(wire({ ToolCall: { name: 'Read' } }))
    projector.accept(wire({ ToolCall: {} }))
    expect(append).toHaveBeenCalledWith(expect.objectContaining({ text: 'Running Read' }))
    expect(append).toHaveBeenCalledWith(expect.objectContaining({ text: 'Running tool' }))
  })

  it('ignores malformed, foreign, empty, and unsupported payloads', () => {
    const { projector, append, updateSession } = harness()
    projector.finishSync(0)
    projector.accept({ invalid: true })
    projector.accept(wire('Running', undefined, null))
    projector.accept(wire('Running', undefined, { hub: ['remote'], agent: 'main' }))
    projector.accept(wire('Running', undefined, { hub: [], agent: 'worker' }))
    projector.accept(wire(42))
    projector.accept(wire(null))
    projector.accept(wire({}))
    projector.accept(wire({ Unknown: {} }))
    projector.accept(wire({ Stream: null }))
    projector.accept(wire({ Stream: { text: 42 } }))
    projector.accept(wire({ ToolCall: null }))
    expect(append).not.toHaveBeenCalled()
    expect(updateSession).not.toHaveBeenCalled()
  })

  it('routes sub-agent attention with independent revisions', () => {
    const { projector, attention } = harness()
    projector.finishSync(20, { worker: 2 })
    projector.accept(wire({ UserQuestionRequest: { id: 'old' } }, 2, {
      hub: [], agent: 'worker',
    }))
    projector.accept(wire({ UserQuestionRequest: { id: 'fresh' } }, 3, {
      hub: [], agent: 'worker',
    }))
    expect(attention).toHaveBeenCalledOnce()
    expect(attention).toHaveBeenCalledWith(
      'question_requested', { id: 'fresh' }, 'worker',
    )
  })

  it('projects modified files from any local agent', () => {
    const { projector, artifacts } = harness()
    projector.finishSync(0, { worker: 0 })
    projector.accept(wire({ TurnDiffSummary: {
      modified_files: ['src/main.rs', 42],
    } }, 1, { hub: [], agent: 'worker' }))
    expect(artifacts).toHaveBeenCalledWith(['src/main.rs'], 'worker')
    projector.accept(wire({ TurnDiffSummary: { modified_files: 'invalid' } }, 2))
    expect(artifacts).toHaveBeenCalledOnce()
  })

  it('keeps event-only lifecycle notices visible for root and child agents', () => {
    const { projector, append, appendAgent, updateSession, updateAgentLifecycle } = harness()
    projector.finishSync(0, { worker: 0 })
    projector.accept(wire({ SessionResumeWarnings: {
      session_id: 'session', warnings: ['scheduler restore failed', 4],
    } }, 1))
    projector.accept(wire('Interrupted', 2))
    projector.accept(wire({ TurnCancelled: { cause: 'parent abort' } }, 3, {
      hub: [], agent: 'worker',
    }))
    expect(append).toHaveBeenCalledWith(expect.objectContaining({
      id: 'event-7', text: 'Session resume warning: scheduler restore failed',
      agentId: 'main', eventNotice: true,
    }))
    expect(append).toHaveBeenCalledWith(expect.objectContaining({
      id: 'event-7', text: 'Turn interrupted.', eventNotice: true,
    }))
    expect(appendAgent).toHaveBeenCalledWith(expect.objectContaining({
      id: 'event-7', text: 'Turn cancelled: parent abort', eventNotice: true,
    }), 'worker')
    expect(updateSession).toHaveBeenLastCalledWith('waiting')

    projector.accept(wire({ Error: { message: 'child failed to start' } }, 4, {
      hub: [], agent: 'worker',
    }))
    expect(appendAgent).toHaveBeenCalledWith(expect.objectContaining({
      role: 'error', text: 'child failed to start', agentId: 'worker',
    }), 'worker')
    expect(updateAgentLifecycle).toHaveBeenCalledWith('worker', 'TurnCancelled', {
      cause: 'parent abort',
    })
    expect(updateAgentLifecycle).toHaveBeenCalledWith('worker', 'Error', {
      message: 'child failed to start',
    })
  })

  it('bounds sync buffering and reports one overflow', () => {
    const { projector, overflow, updateSession } = harness()
    for (let index = 0; index < 70; index += 1) projector.accept(wire('Running', index + 1))
    projector.finishSync(0)
    expect(overflow).toHaveBeenCalledOnce()
    expect(updateSession).not.toHaveBeenCalled()
    projector.beginSync()
    projector.accept(wire('Running', 100))
    projector.finishSync(99)
    expect(updateSession).toHaveBeenCalledOnce()
  })

  it('resyncs when authoritative history is newer than the snapshot', () => {
    const { projector, overflow } = harness()
    projector.accept(wire({ SessionHistoryLoaded: { messages: [] } }, 4))
    projector.finishSync(3)
    expect(overflow).toHaveBeenCalledOnce()
    projector.accept(wire({ SessionHistoryLoaded: { messages: [] } }, 4))
    expect(overflow).toHaveBeenCalledOnce()
  })
})
