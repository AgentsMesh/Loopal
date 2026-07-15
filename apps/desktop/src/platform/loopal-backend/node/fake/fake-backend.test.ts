import { describe, expect, it, vi } from 'vitest'
import { CancellationToken, CancellationTokenSource } from '../../../../base/common/cancellation'
import { FakeDesktopBackend, type FakeBackendClock } from './fake-backend'

function clock(): FakeBackendClock {
  return {
    now: () => new Date('2026-07-11T12:00:00.000Z'),
    delay: async () => undefined,
  }
}

describe('FakeDesktopBackend', () => {
  it('supports its real system clock defaults', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-07-11T13:00:00.000Z'))
    try {
      const backend = new FakeDesktopBackend()
      const pending = backend.sendMessage('session-desktop', 'Use the real clock adapter')
      await vi.advanceTimersByTimeAsync(15)
      await pending
      const result = await backend.openSession('session-desktop')
      expect(result.session.updatedAt).toBe('2026-07-11T13:00:00.015Z')
      backend.dispose()
    } finally {
      vi.useRealTimers()
    }
  })

  it('returns deterministic split session and runtime catalogs', async () => {
    const backend = new FakeDesktopBackend(clock())
    const bootstrap = await backend.bootstrap()
    expect(bootstrap).toMatchObject({ protocolVersion: 2, activeSessionId: 'session-desktop' })
    expect(bootstrap.sessions).toHaveLength(3)
    expect(bootstrap.runtimes).toHaveLength(2)
    const complete = await backend.openSession('session-audit')
    expect(complete.artifacts[0]).toMatchObject({
      sessionId: 'session-audit', title: 'Architecture findings.md',
    })
    backend.dispose()
  })

  it('keeps conversations isolated and emits scoped entries', async () => {
    const backend = new FakeDesktopBackend(clock())
    const protocolBefore = await backend.openSession('session-protocol')
    const events: unknown[] = []
    backend.onEvent((event) => events.push(event))
    await backend.sendMessage('session-desktop', 'Add protocol tests')
    expect(events).toEqual(expect.arrayContaining([
      expect.objectContaining({ type: 'conversation_entry', sessionId: 'session-desktop' }),
      expect.objectContaining({
        type: 'artifact_created', artifact: expect.objectContaining({
          sessionId: 'session-desktop', producerAgentId: 'agent-root',
        }),
      }),
    ]))
    const desktop = await backend.openSession('session-desktop')
    const protocolAfter = await backend.openSession('session-protocol')
    expect(desktop.conversation.at(-1)?.role).toBe('assistant')
    expect(protocolAfter.conversation).toEqual(protocolBefore.conversation)
    backend.dispose()
  })

  it('routes messages and artifacts into a selected child conversation', async () => {
    const backend = new FakeDesktopBackend(clock())
    const events: unknown[] = []
    backend.onEvent((event) => events.push(event))
    await backend.sendMessage(
      'session-desktop', 'Verify child routing', CancellationToken.None, 'agent-e2e',
    )
    const detail = await backend.openSession('session-desktop')
    const child = detail.agents.find((agent) => agent.id === 'agent-e2e')
    expect(child?.conversation?.map((entry) => entry.text)).toEqual(expect.arrayContaining([
      'Verify child routing', 'Loopal handled this message inside the selected session runtime.',
    ]))
    expect(events).toContainEqual(expect.objectContaining({
      type: 'artifact_created', artifact: expect.objectContaining({
        producerAgentId: 'agent-e2e',
      }),
    }))
    await expect(backend.sendMessage(
      'session-desktop', 'missing', CancellationToken.None, 'retired-agent',
    )).rejects.toThrow('Agent is not available')
    backend.dispose()
  })

  it('creates, stops, and restarts with increasing generations', async () => {
    const backend = new FakeDesktopBackend(clock())
    const selected = await backend.authorizeSessionDirectory(process.cwd())
    const created = await backend.createSession({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })
    expect(created.session).toMatchObject({ status: 'running', activeRuntimeId: expect.any(String) })
    await backend.stopSession(created.session.id)
    const stopped = await backend.openSession(created.session.id)
    expect(stopped.session.status).toBe('stopped')
    expect(stopped.session.activeRuntimeId).toBeUndefined()
    const second = await backend.restartSession(created.session.id)
    const third = await backend.restartSession(created.session.id)
    expect([second.generation, third.generation]).toEqual([2, 3])
    expect((await backend.openSession(created.session.id)).session.activeRuntimeId).toBe(third.id)
    await backend.stopSession('session-audit')
    backend.dispose()
  })

  it('rejects unknown sessions and cancellation', async () => {
    const backend = new FakeDesktopBackend(clock())
    await expect(backend.openSession('missing')).rejects.toThrow('Session not found')
    await expect(backend.sendMessage('missing', 'hello')).rejects.toThrow('Session not found')
    await expect(backend.createSession({ authorizationId: '5d0c638c-d44c-4f47-818b-62e6b599e31c', launchMode: 'directory' })).rejects.toThrow('authorization')
    const source = new CancellationTokenSource()
    source.cancel()
    await expect(backend.bootstrap(source.token)).rejects.toThrow('cancelled')
    await expect(backend.openSession('session-desktop', source.token)).rejects.toThrow('cancelled')
    backend.dispose()
  })

  it('observes cancellation after asynchronous work starts and returns clones', async () => {
    let release: (() => void) | undefined
    const backend = new FakeDesktopBackend({
      now: clock().now,
      delay: () => new Promise<void>((resolve) => { release = resolve }),
    })
    const source = new CancellationTokenSource()
    const pending = backend.sendMessage('session-desktop', 'cancel me', source.token)
    source.cancel()
    release?.()
    await expect(pending).rejects.toThrow('cancelled')
    const detail = await backend.openSession('session-desktop')
    detail.conversation.length = 0
    expect((await backend.openSession('session-desktop')).conversation.length).toBeGreaterThan(0)
    backend.dispose()
  })

  it('projects core control actions through authoritative detail events', async () => {
    const backend = new FakeDesktopBackend(clock())
    const runtime = (await backend.bootstrap()).runtimes.find((item) => (
      item.sessionId === 'session-desktop'
    ))!
    const target = {
      sessionId: runtime.sessionId, runtimeId: runtime.id,
      generation: runtime.generation, agentId: runtime.rootAgent,
    }
    const events: unknown[] = []
    backend.onEvent((event) => events.push(event))
    await backend.controlAgent({ target, command: { type: 'mode', mode: 'plan' } })
    expect((await backend.openSession(target.sessionId)).agents[0]?.mode).toBe('plan')
    await backend.controlAgent({ target, command: { type: 'compact' } })
    expect((await backend.openSession(target.sessionId)).view?.compactBanner)
      .toBe('Summarizing conversation context.')
    await backend.controlAgent({ target, command: { type: 'suspend' } })
    expect((await backend.openSession(target.sessionId)).agents[0]?.status).toBe('suspended')
    await backend.controlAgent({ target, command: { type: 'unsuspend' } })
    await backend.interruptAgent(target)
    expect((await backend.openSession(target.sessionId)).agents[0]?.status).toBe('waiting')
    await backend.controlAgent({ target, command: { type: 'clear' } })
    expect((await backend.openSession(target.sessionId)).conversation).toEqual([])
    expect(events).toEqual(expect.arrayContaining([
      expect.objectContaining({ type: 'session_detail_replaced' }),
      expect.objectContaining({ type: 'session_updated' }),
    ]))
    backend.dispose()
  })

  it('supports resource controls and rejects stale or unknown targets', async () => {
    const backend = new FakeDesktopBackend(clock())
    const runtime = (await backend.bootstrap()).runtimes.find((item) => (
      item.sessionId === 'session-desktop'
    ))!
    const target = {
      sessionId: runtime.sessionId, runtimeId: runtime.id,
      generation: runtime.generation, agentId: runtime.rootAgent,
    }
    for (const command of [
      { type: 'model', model: 'gpt-5-mini' } as const,
      { type: 'thinking', config: { type: 'effort', level: 'high' } } as const,
      { type: 'permission', mode: 'ask_any_write' } as const,
      { type: 'decision', mode: 'classifier' } as const,
      { type: 'sandbox', policy: 'read_only' } as const,
      { type: 'mcp_disconnect', server: 'filesystem' } as const,
      { type: 'mcp_reconnect', server: 'filesystem' } as const,
      { type: 'background_task_kill', id: 'bg-bazel' } as const,
      { type: 'cron_delete', id: 'cron-health' } as const,
    ]) await backend.controlAgent({ target, command })
    const detail = await backend.openSession(target.sessionId)
    expect(detail.agents[0]).toMatchObject({
      model: 'gpt-5-mini', thinkingConfig: 'high', permissionMode: 'ask_any_write',
      decisionMode: 'classifier', sandboxPolicy: 'read_only',
    })
    expect(detail.view?.backgroundTasks[0]?.status).toBe('killed')
    expect(detail.view?.crons).toEqual([])
    expect(detail.view?.mcpServers[0]?.status).toBe('ready')
    await expect(backend.interruptAgent({ ...target, generation: 9 }))
      .rejects.toMatchObject({ code: 'RUNTIME_GONE' })
    await expect(backend.interruptAgent({ ...target, agentId: 'completed' }))
      .rejects.toMatchObject({ code: 'AGENT_GONE' })
    backend.dispose()
  })
})
