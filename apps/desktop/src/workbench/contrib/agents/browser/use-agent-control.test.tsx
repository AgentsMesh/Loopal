import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { type AgentControlDisposition } from '../../../../shared/contracts'
import {
  createTestAPI,
  sessionDetail,
  sessionOne,
  updatedAt,
} from '../../../../../test/support/workbench/api-stub'
import { type DesktopProjection } from '../../../browser/desktop-event-projector'
import { useAgentControl } from './use-agent-control'

const runtime = {
  id: 'runtime-1', sessionId: sessionOne.id, workspaceId: sessionOne.workspaceId,
  generation: 4, state: 'ready' as const, rootAgent: 'agent-session-1', startedAt: updatedAt,
}
const detail = sessionDetail(sessionOne)
const projection: DesktopProjection = {
  hostStatus: 'ready', sessions: [sessionOne], runtimes: [runtime], detail,
}
const target = {
  sessionId: sessionOne.id, runtimeId: runtime.id,
  generation: runtime.generation, agentId: runtime.rootAgent,
}

describe('useAgentControl', () => {
  it('builds an exact runtime target for control and interrupt', async () => {
    const controlAgent = vi.fn(async () => ({ status: 'applied' as const }))
    const interruptAgent = vi.fn(async () => undefined)
    const { api } = createTestAPI({ controlAgent, interruptAgent })
    const hook = renderHook(() => useAgentControl(api, projection))
    expect(hook.result.current.available(runtime.rootAgent)).toBe(true)
    expect(hook.result.current.available('missing')).toBe(false)
    let controlled: boolean | undefined
    await act(async () => {
      controlled = await hook.result.current.control(
        runtime.rootAgent, { type: 'mode', mode: 'plan' },
      )
    })
    expect(controlled).toBe(true)
    expect(controlAgent).toHaveBeenCalledWith({
      target, command: { type: 'mode', mode: 'plan' },
    })
    await act(async () => hook.result.current.interrupt(runtime.rootAgent))
    expect(interruptAgent).toHaveBeenCalledWith(target)
    expect(hook.result.current.error).toBeUndefined()
  })

  it('rejects incomplete projections before crossing preload', async () => {
    const controlAgent = vi.fn(async () => ({ status: 'applied' as const }))
    const { api } = createTestAPI({ controlAgent })
    const { detail: _detail, ...projectionWithoutDetail } = projection
    const hook = renderHook(
      ({ value }: { value: DesktopProjection }) => useAgentControl(api, value),
      { initialProps: { value: projectionWithoutDetail } },
    )
    let controlled: boolean | undefined
    await act(async () => {
      controlled = await hook.result.current.control(runtime.rootAgent, { type: 'clear' })
    })
    expect(controlled).toBe(false)
    expect(hook.result.current.error).toContain('no longer has a live runtime')
    expect(controlAgent).not.toHaveBeenCalled()
    for (const value of [
      { ...projection, runtimes: [] },
      { ...projection, runtimes: [{ ...runtime, sessionId: 'other' }] },
      { ...projection, runtimes: [{ ...runtime, state: 'stopped' as const }] },
      { ...projection, detail: { ...detail, agents: [] } },
      { ...projection, detail: {
        ...detail, agents: detail.agents.map((agent) => ({
          ...agent, controllable: false,
        })),
      } },
      { ...projection, detail: {
        ...detail, agents: detail.agents.map((agent) => ({
          ...agent, status: 'completed' as const,
        })),
      } },
      { ...projection, detail: {
        ...detail, agents: detail.agents.map((agent) => ({
          ...agent, status: 'failed' as const,
        })),
      } },
    ]) {
      hook.rerender({ value })
      expect(hook.result.current.available(runtime.rootAgent)).toBe(false)
    }
  })

  it('reports failures and suppresses concurrent commands', async () => {
    let release!: () => void
    const pending = new Promise<{ status: 'applied' }>((resolve) => {
      release = () => resolve({ status: 'applied' })
    })
    const controlAgent = vi.fn(() => pending)
    const { api } = createTestAPI({ controlAgent })
    const hook = renderHook(() => useAgentControl(api, projection))
    let first!: Promise<boolean>
    act(() => { first = hook.result.current.control(runtime.rootAgent, { type: 'clear' }) })
    expect(hook.result.current.busy).toBe(true)
    let suppressed: boolean | undefined
    await act(async () => {
      suppressed = await hook.result.current.control(runtime.rootAgent, { type: 'suspend' })
    })
    expect(suppressed).toBe(false)
    expect(controlAgent).toHaveBeenCalledOnce()
    release()
    await act(async () => first)
    expect(hook.result.current.busy).toBe(false)

    controlAgent.mockRejectedValueOnce('denied')
    await act(async () => hook.result.current.control(runtime.rootAgent, { type: 'clear' }))
    expect(hook.result.current.error).toBe('denied')
    controlAgent.mockRejectedValueOnce(new Error('closed'))
    await act(async () => hook.result.current.control(runtime.rootAgent, { type: 'clear' }))
    expect(hook.result.current.error).toBe('closed')
  })

  it('keeps accepted and indeterminate dispositions distinct from rejection', async () => {
    let next: AgentControlDisposition = { status: 'queued' }
    const controlAgent = vi.fn(async () => next)
    const { api } = createTestAPI({ controlAgent })
    const hook = renderHook(() => useAgentControl(api, projection))

    await act(async () => {
      await expect(hook.result.current.control(runtime.rootAgent, { type: 'suspend' }))
        .resolves.toBe(true)
    })
    expect(hook.result.current.disposition).toEqual({ status: 'queued' })
    expect(hook.result.current.error).toBeUndefined()

    next = { status: 'unknown' }
    await act(async () => {
      await expect(hook.result.current.control(runtime.rootAgent, { type: 'clear' }))
        .resolves.toBe(true)
    })
    expect(hook.result.current.disposition).toEqual({ status: 'unknown' })
    expect(hook.result.current.error).toBeUndefined()

    next = { status: 'rejected', reason: 'unsupported control' }
    await act(async () => {
      await expect(hook.result.current.control(runtime.rootAgent, { type: 'clear' }))
        .resolves.toBe(false)
    })
    expect(hook.result.current.disposition).toEqual(next)
    expect(hook.result.current.error).toBe('unsupported control')
  })
})
