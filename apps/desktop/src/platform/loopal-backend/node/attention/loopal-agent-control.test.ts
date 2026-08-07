import { describe, expect, it, vi } from 'vitest'
import { CancellationToken, CancellationTokenSource } from '../../../../base/common/cancellation'
import { type AgentControlTarget } from '../../../../shared/contracts'
import { LoopalAgentControl, type AgentControlRouter } from './loopal-agent-control'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

const target: AgentControlTarget = {
  sessionId: 'session', runtimeId: 'runtime', generation: 3, agentId: 'main',
}

function fixture(state = 'connected') {
  const request = vi.fn<(
    method: string, params?: unknown, signal?: AbortSignal,
  ) => Promise<unknown>>(async (method) => {
    if (method === 'hub/list_agents') return { agents: [{ name: 'main', state }] }
    if (method === 'meta/topology') return {
      hubs: [{
        hub: 'hub-b',
        topology: { agents: [{
          name: 'worker', parent: 'main', children: [], lifecycle: 'running',
        }] },
      }],
    }
    return { status: 'applied' }
  })
  const runtime = {
    sessionId: 'session', runtimeId: 'runtime', generation: 3, workspaceId: 'workspace',
    host: { currentStatus: 'ready', request },
  } as unknown as SessionRuntimeHandle
  let current: { runtime: SessionRuntimeHandle } | undefined = { runtime }
  const router: AgentControlRouter = { session: () => current }
  return {
    service: new LoopalAgentControl(router), request, runtime,
    retire: () => { current = undefined },
  }
}

function controlResponse(response: unknown, rejects = false) {
  const value = fixture()
  value.request.mockResolvedValueOnce({ agents: [{ name: 'main', state: 'connected' }] })
  if (rejects) value.request.mockRejectedValueOnce(response)
  else value.request.mockResolvedValueOnce(response)
  return value.service.controlAgent({ target, command: { type: 'clear' } })
}

describe('LoopalAgentControl', () => {
  it('validates the live agent then routes typed control and interrupt calls', async () => {
    const value = fixture()
    await expect(value.service.controlAgent({
      target, command: { type: 'mode', mode: 'plan' },
    }, CancellationToken.None)).resolves.toEqual({ status: 'applied' })
    expect(value.request).toHaveBeenNthCalledWith(1, 'hub/list_agents', {}, expect.any(AbortSignal))
    expect(value.request).toHaveBeenNthCalledWith(2, 'hub/control', {
      target: 'main', command: { ModeSwitch: 'Plan' },
    }, expect.any(AbortSignal))
    await value.service.interruptAgent(target, CancellationToken.None)
    expect(value.request).toHaveBeenLastCalledWith(
      'hub/interrupt', { target: 'main' }, expect.any(AbortSignal),
    )
  })

  it('rejects every stale runtime scope without touching its Host', async () => {
    for (const stale of [
      { ...target, sessionId: 'other' },
      { ...target, runtimeId: 'old' },
      { ...target, generation: 2 },
    ]) {
      const value = fixture()
      await expect(value.service.interruptAgent(stale, CancellationToken.None))
        .rejects.toMatchObject({ code: 'RUNTIME_GONE' })
      expect(value.request).not.toHaveBeenCalled()
    }
    const value = fixture()
    value.retire()
    await expect(value.service.controlAgent({
      target, command: { type: 'clear' },
    }, CancellationToken.None)).rejects.toMatchObject({ code: 'RUNTIME_GONE' })
    expect(value.request).not.toHaveBeenCalled()
  })

  it('accepts local/connected agents and rejects missing or shadow agents', async () => {
    const local = fixture('local')
    await expect(local.service.controlAgent({
      target, command: { type: 'suspend' },
    }, CancellationToken.None)).resolves.toEqual({ status: 'applied' })
    for (const state of ['shadow', 'finished']) {
      const value = fixture(state)
      await expect(value.service.controlAgent({
        target, command: { type: 'clear' },
      }, CancellationToken.None)).rejects.toMatchObject({ code: 'AGENT_GONE' })
      expect(value.request).toHaveBeenCalledOnce()
    }
  })

  it('controls only live remote Agents confirmed by the current MetaHub topology', async () => {
    const value = fixture()
    const remote = { ...target, agentId: 'hub-b/worker' }
    await expect(value.service.controlAgent({
      target: remote, command: { type: 'clear' },
    })).resolves.toEqual({ status: 'applied' })
    expect(value.request).toHaveBeenNthCalledWith(
      2, 'meta/topology', {}, expect.any(AbortSignal),
    )
    expect(value.request).toHaveBeenNthCalledWith(
      3, 'hub/control', { target: 'hub-b/worker', command: 'Clear' },
      expect.any(AbortSignal),
    )

    const missing = fixture()
    await expect(missing.service.interruptAgent({
      ...target, agentId: 'hub-b/guessed',
    })).rejects.toMatchObject({ code: 'AGENT_GONE' })
    expect(missing.request).toHaveBeenCalledTimes(2)
  })

  it('rechecks ownership after agent lookup and honors cancellation', async () => {
    const value = fixture()
    value.request.mockImplementationOnce(async () => {
      value.retire()
      return { agents: [{ name: 'main', state: 'connected' }] }
    })
    await expect(value.service.controlAgent({
      target, command: { type: 'clear' },
    }, CancellationToken.None)).rejects.toMatchObject({ code: 'RUNTIME_GONE' })
    expect(value.request).toHaveBeenCalledOnce()

    const cancelled = fixture()
    await expect(cancelled.service.interruptAgent(target, CancellationToken.Cancelled))
      .rejects.toThrow('cancelled')
    expect(cancelled.request).not.toHaveBeenCalled()
  })

  it('aborts an in-flight Host lookup when its channel call is cancelled', async () => {
    const value = fixture()
    const source = new CancellationTokenSource()
    value.request.mockImplementationOnce(async (_method, _params, signal) => (
      await new Promise<never>((_resolve, reject) => {
        signal!.addEventListener('abort', () => reject(new Error('aborted')), { once: true })
      })
    ))
    const pending = value.service.interruptAgent(target, source.token)
    await vi.waitFor(() => expect(value.request).toHaveBeenCalledOnce())
    source.cancel()
    await expect(pending).rejects.toThrow('aborted')
    source.dispose()
  })

  it('preserves typed, legacy, rejected, and indeterminate dispositions', async () => {
    for (const [acknowledgement, expected] of [
      [{ status: 'applied' }, { status: 'applied' }],
      [{ status: 'queued' }, { status: 'queued' }],
      [{ status: 'unknown' }, { status: 'unknown' }],
      [
        { status: 'rejected', reason: 'unsupported mode' },
        { status: 'rejected', reason: 'unsupported mode' },
      ],
      [{ ok: true }, { status: 'queued' }],
      [{ ok: false, error: 'old rejection' }, { status: 'rejected', reason: 'old rejection' }],
    ] as const) {
      await expect(controlResponse(acknowledgement)).resolves.toEqual(expected)
    }

    for (const acknowledgement of [null, [], 'applied', { ok: 'true' }]) {
      await expect(controlResponse(acknowledgement))
        .rejects.toMatchObject({ code: 'CONTROL_PROTOCOL' })
    }

    const oldRejection = new Error("control rejected: decision mode 'agent' is not implemented")
    await expect(controlResponse(oldRejection, true)).resolves.toEqual({
      status: 'rejected', reason: "decision mode 'agent' is not implemented",
    })

    await expect(controlResponse(
      new Error('Loopal Hub request timed out: hub/control'), true,
    )).resolves.toEqual({ status: 'unknown' })

    await expect(controlResponse('remote rejected', true)).rejects.toMatchObject({
      code: 'CONTROL_FAILED', message: expect.stringContaining('remote rejected'),
    })
  })

  it('preserves cancellation while the runtime is applying a control', async () => {
    const value = fixture()
    const source = new CancellationTokenSource()
    value.request.mockResolvedValueOnce({ agents: [{ name: 'main', state: 'connected' }] })
      .mockImplementationOnce(async (_method, _params, signal) => (
        await new Promise<never>((_resolve, reject) => {
          signal!.addEventListener('abort', () => reject(new Error('aborted')), { once: true })
        })
      ))
    const pending = value.service.controlAgent({
      target, command: { type: 'mode', mode: 'plan' },
    }, source.token)
    await vi.waitFor(() => expect(value.request).toHaveBeenCalledTimes(2))
    source.cancel()
    await expect(pending).rejects.toThrow('aborted')
    source.dispose()
  })
})
