import { describe, expect, it } from 'vitest'
import { sessionDetail, sessionOne, sessionTwo, updatedAt } from '../../../test/support/workbench/api-stub'
import { projectDesktopEvent, type DesktopProjection } from './desktop-event-projector'

const runtime = {
  id: 'runtime-1', sessionId: sessionOne.id, workspaceId: sessionOne.workspaceId,
  generation: 1, state: 'ready' as const, rootAgent: 'main', startedAt: updatedAt,
}

function projection(): DesktopProjection {
  return {
    hostStatus: 'ready', sessions: [sessionOne, sessionTwo], runtimes: [runtime],
    detail: sessionDetail(sessionOne),
  }
}

describe('desktop event projection', () => {
  it('keeps conversations and artifacts isolated by session', () => {
    const current = projection()
    const entry = { id: 'other', role: 'assistant' as const, text: 'other', createdAt: updatedAt }
    const ignored = projectDesktopEvent(current, {
      type: 'conversation_entry', sessionId: sessionTwo.id, entry,
    })
    expect(ignored.detail?.conversation).toEqual(current.detail?.conversation)
    const accepted = projectDesktopEvent(current, {
      type: 'conversation_entry', sessionId: sessionOne.id, entry,
    })
    expect(accepted.detail?.conversation.at(-1)).toEqual(entry)
    const artifact = {
      id: 'file', sessionId: sessionOne.id, title: 'main.rs', kind: 'code' as const,
      uri: 'loopal-workspace://src%2Fmain.rs', mediaType: 'text/plain',
      producerAgentId: 'worker', createdAt: updatedAt,
    }
    const withArtifact = projectDesktopEvent(current, { type: 'artifact_created', artifact })
    const repeated = projectDesktopEvent(withArtifact, { type: 'artifact_created', artifact })
    expect(repeated.detail?.artifacts).toEqual([artifact])
    expect(projectDesktopEvent(current, {
      type: 'artifact_created', artifact: { ...artifact, sessionId: sessionTwo.id },
    })).toBe(current)
  })

  it('bounds retired runtimes and ignores late obsolete generations', () => {
    let current = projectDesktopEvent(projection(), {
      type: 'runtime_updated', runtime: { ...runtime, state: 'stopped' },
    })
    const next = {
      ...runtime, id: 'runtime-2', generation: 2, state: 'ready' as const,
    }
    current = projectDesktopEvent(current, { type: 'runtime_updated', runtime: next })
    expect(current.runtimes).toEqual([next])
    current = projectDesktopEvent(current, {
      type: 'runtime_updated', runtime: { ...runtime, state: 'crashed' },
    })
    expect(current.runtimes).toEqual([next])
  })
})
