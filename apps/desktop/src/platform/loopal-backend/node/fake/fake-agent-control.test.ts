import { describe, expect, it } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import { type AgentControlCommand } from '../../../../shared/contracts'
import { FakeDesktopBackend } from './fake-backend'

async function richTarget(backend: FakeDesktopBackend) {
  const runtime = (await backend.bootstrap()).runtimes.find((candidate) => (
    candidate.sessionId === 'session-desktop'
  ))!
  return {
    sessionId: runtime.sessionId, runtimeId: runtime.id,
    generation: runtime.generation, agentId: runtime.rootAgent,
  }
}

describe('FakeAgentControl edges', () => {
  it('projects thinking, rewind, and status commands', async () => {
    const backend = new FakeDesktopBackend()
    const target = await richTarget(backend)
    const commands: AgentControlCommand[] = [
      { type: 'thinking', config: { type: 'auto' } },
      { type: 'thinking', config: { type: 'disabled' } },
      { type: 'thinking', config: { type: 'budget', tokens: 4_096 } },
      { type: 'rewind', turnIndex: 1 },
      { type: 'mcp_status' },
    ]
    for (const command of commands) await backend.controlAgent({ target, command })
    const detail = await backend.openSession(target.sessionId)
    expect(detail.agents[0]?.thinkingConfig).toBe('4096 tokens')
    expect(detail.conversation).toHaveLength(1)
    expect(detail.view?.goal).toMatchObject({ status: 'active' })
    backend.dispose()
  })

  it('leaves unknown resource IDs unchanged and tolerates sessions without a rich view', async () => {
    const backend = new FakeDesktopBackend()
    const target = await richTarget(backend)
    for (const command of [
      { type: 'mcp_disconnect', server: 'missing' },
      { type: 'background_task_kill', id: 'missing' },
      { type: 'cron_delete', id: 'missing' },
    ] as const) await backend.controlAgent({ target, command })
    const detail = await backend.openSession(target.sessionId)
    expect(detail.view?.mcpServers[0]?.status).toBe('ready')
    expect(detail.view?.backgroundTasks[0]?.status).toBe('running')
    expect(detail.view?.crons).toHaveLength(1)

    backend.dispose()
  })

  it('rejects every malformed live identity and cancellation', async () => {
    const backend = new FakeDesktopBackend()
    const target = await richTarget(backend)
    for (const stale of [
      { ...target, sessionId: 'missing' },
      { ...target, runtimeId: 'missing' },
      { ...target, generation: target.generation + 1 },
    ]) await expect(backend.controlAgent({
      target: stale, command: { type: 'clear' },
    })).rejects.toMatchObject({ code: 'RUNTIME_GONE' })
    await expect(backend.controlAgent(
      { target, command: { type: 'clear' } }, CancellationToken.Cancelled,
    )).rejects.toThrow('cancelled')
    backend.dispose()
  })
})
