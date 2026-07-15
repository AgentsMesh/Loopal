import { WorkspaceScopedHost } from './workspace-scoped-host'
import { FakeHost } from '../backend/loopal-backend.test-fixtures'

describe('WorkspaceScopedHost', () => {
  it('translates app workspace IDs at every Host request and event boundary', async () => {
    const raw = new FakeHost('session-1', [])
    raw.request.mockImplementation(async (_method, params) => ({
      workspaceId: (params as { workspaceId: string }).workspaceId,
    }))
    const host = new WorkspaceScopedHost(raw, 'local-dynamic')
    const activate = vi.fn(async () => undefined)
    await host.start(activate)
    expect(raw.start).toHaveBeenCalledWith(activate)
    await expect(host.request('workspace/listDirectory', {
      workspaceId: 'local-dynamic', path: '',
    })).resolves.toEqual({ workspaceId: 'local-dynamic' })
    expect(raw.request).toHaveBeenCalledWith(
      'workspace/listDirectory', { workspaceId: 'local-workspace', path: '' }, undefined,
    )

    const events: unknown[] = []
    host.onNotification((event) => events.push(event.params))
    raw.notification('workspace/fileChanged', { workspaceId: 'local-workspace', path: 'a.ts' })
    expect(events).toEqual([{ workspaceId: 'local-dynamic', path: 'a.ts' }])
  })
})
