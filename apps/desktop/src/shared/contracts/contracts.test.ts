import { describe, expect, it } from 'vitest'
import {
  CreateSessionInputSchema, DesktopEventSchema, DesktopImageAttachmentListSchema,
  SendMessageInputSchema, SessionDetailSchema,
  SessionOperationInputSchema, WorkbenchBootstrapSchema,
} from './'

describe('desktop session contracts', () => {
  const now = '2026-07-11T12:00:00.000Z'
  const session = {
    id: 'session-1', workspaceId: 'workspace-1', title: 'Session',
    model: 'gpt-5', mode: 'agent', status: 'running' as const,
    createdAt: now, updatedAt: now, activeRuntimeId: 'runtime-1',
  }
  const runtime = {
    id: 'runtime-1', sessionId: session.id, workspaceId: session.workspaceId,
    generation: 1, state: 'ready' as const, rootAgent: 'main', startedAt: now,
  }

  it('validates protocol v2 session and runtime catalogs', () => {
    const bootstrap = WorkbenchBootstrapSchema.parse({
      protocolVersion: 2, hostStatus: 'ready',
      workspaces: [{
        id: 'workspace-1', name: 'Loopal', rootUri: 'file:///loopal', kind: 'folder',
      }],
      sessions: [session], runtimes: [runtime], activeSessionId: session.id,
    })
    expect(bootstrap.sessions).toHaveLength(1)
    expect(bootstrap.runtimes).toEqual([runtime])
    expect(SessionDetailSchema.parse({
      session, conversation: [], agents: [], artifacts: [],
    }).session).toEqual(session)
  })

  it('validates scoped session and runtime events', () => {
    const detail = { session, conversation: [], agents: [], artifacts: [] }
    const events = [
      { type: 'host_status', status: 'ready' },
      { type: 'session_updated', session },
      { type: 'runtime_updated', runtime },
      { type: 'session_detail_replaced', detail },
      {
        type: 'conversation_entry', sessionId: session.id,
        entry: { id: 'message-1', role: 'assistant', text: 'done', createdAt: now },
      },
      {
        type: 'artifact_created',
        artifact: {
          id: 'artifact-1', sessionId: session.id, title: 'Report', kind: 'report',
          uri: 'loopal-artifact://report', mediaType: 'text/markdown',
          producerAgentId: 'agent-1', createdAt: now,
        },
      },
    ]
    for (const event of events) expect(DesktopEventSchema.parse(event)).toEqual(event)
  })

  it('rejects obsolete task-shaped and malformed inputs', () => {
    expect(SessionOperationInputSchema.safeParse({ sessionId: '' }).success).toBe(false)
    expect(SendMessageInputSchema.safeParse({ sessionId: 'x', text: '   ' }).success).toBe(false)
    expect(SendMessageInputSchema.parse({
      sessionId: 'x', text: 'hello', agentId: 'worker',
    })).toMatchObject({ agentId: 'worker' })
    expect(SendMessageInputSchema.safeParse({
      sessionId: 'x', text: 'hello', agentId: '',
    }).success).toBe(false)
    const image = {
      name: 'pixel.png', mediaType: 'image/png', data: 'iVBORw==', sizeBytes: 4,
    }
    expect(SendMessageInputSchema.parse({ sessionId: 'x', text: '', images: [image] }))
      .toMatchObject({ images: [image] })
    expect(DesktopImageAttachmentListSchema.safeParse([
      { ...image, sizeBytes: 5 },
    ]).success).toBe(false)
    expect(WorkbenchBootstrapSchema.safeParse({
      protocolVersion: 1, workspaces: [], tasks: [],
    }).success).toBe(false)
    expect(DesktopEventSchema.safeParse({ type: 'task_updated', task: session }).success)
      .toBe(false)
  })

  it('accepts only strict opaque session-directory launch inputs', () => {
    const authorizationId = '5d0c638c-d44c-4f47-818b-62e6b599e31c'
    expect(CreateSessionInputSchema.safeParse({
      authorizationId, launchMode: 'directory',
    }).success).toBe(true)
    expect(CreateSessionInputSchema.safeParse({
      authorizationId, launchMode: 'worktree', worktreeName: 'desktop-1',
    }).success).toBe(true)
    for (const input of [
      { authorizationId, launchMode: 'directory', cwd: '/injected' },
      { authorizationId, launchMode: 'directory', path: '/injected' },
      { authorizationId, launchMode: 'worktree', worktreeName: 'wt', cwd: '/injected' },
      { workspaceId: 'workspace' },
      { workspaceId: 'workspace', cwd: '/injected' },
    ]) expect(CreateSessionInputSchema.safeParse(input).success).toBe(false)
  })
})
