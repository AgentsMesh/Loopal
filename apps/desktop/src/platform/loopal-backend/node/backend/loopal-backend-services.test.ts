import { describe, expect, it, vi } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import { type DesktopEvent } from '../../../../shared/contracts'
import { type DesktopHostClient } from './loopal-backend-types'
import { LoopalBackendServices } from './loopal-backend-services'
import { type LoopalSessionDirectory } from '../sessions/loopal-session-directory'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

const request = vi.fn()
const host = { currentStatus: 'ready', request } as unknown as DesktopHostClient
const runtime: SessionRuntimeHandle = {
  workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime', generation: 1, host,
}

function notification(method: string, params: unknown) {
  return { ...runtime, method, params }
}

describe('LoopalBackendServices', () => {
  it('filters nonleaders and malformed service events', () => {
    let leader = false
    const directory = {
      leaders: { isLeader: vi.fn(() => leader) },
    } as unknown as LoopalSessionDirectory
    const events: DesktopEvent[] = []
    const services = new LoopalBackendServices({
      workspace: async () => runtime,
      liveSession: async () => runtime,
    }, directory, (event) => events.push(event))
    expect(services.operations().gitStatus).toBeTypeOf('function')

    services.accept(notification('workspace/fileChanged', {
      workspaceId: 'workspace', path: 'ignored.rs', kind: 'changed',
    }))
    leader = true
    services.accept(notification('workspace/unknown', {}))
    services.accept(notification('workspace/fileChanged', {
      workspaceId: 'workspace', path: 'main.rs', kind: 'changed',
    }))
    expect(events).toEqual([{
      type: 'file_changed', workspaceId: 'workspace', path: 'main.rs', kind: 'changed',
    }])
  })

  it('binds exact live-runtime agent controls without resuming a session', async () => {
    const directory = {
      leaders: { isLeader: () => true },
      runtimeForSession: vi.fn(() => runtime),
    } as unknown as LoopalSessionDirectory
    const services = new LoopalBackendServices({
      workspace: async () => runtime,
      liveSession: async () => runtime,
    }, directory, vi.fn())
    request.mockImplementation(async (method: string) => method === 'hub/list_agents'
      ? { agents: [{ name: 'main', state: 'connected' }] }
      : { status: 'applied' })
    await services.operations().controlAgent({
      target: {
        sessionId: 'session', runtimeId: 'runtime', generation: 1, agentId: 'main',
      },
      command: { type: 'clear' },
    }, CancellationToken.None)
    expect(directory.runtimeForSession).toHaveBeenCalledWith('session')
    expect(request).toHaveBeenLastCalledWith(
      'hub/control', { target: 'main', command: 'Clear' }, expect.any(AbortSignal),
    )
    vi.mocked(directory.runtimeForSession).mockReturnValue(undefined)
    await expect(services.operations().interruptAgent({
      sessionId: 'session', runtimeId: 'runtime', generation: 1, agentId: 'main',
    }, CancellationToken.None)).rejects.toMatchObject({ code: 'RUNTIME_GONE' })
  })
})
