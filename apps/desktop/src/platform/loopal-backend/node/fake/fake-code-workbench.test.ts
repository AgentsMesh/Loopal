import { CancellationToken } from '../../../../base/common/cancellation'
import { FakeDesktopBackend } from './fake-backend'

describe('Fake code workbench binding', () => {
  it('exposes workspace services through DesktopBackend', async () => {
    const backend = new FakeDesktopBackend()
    const bootstrap = await backend.bootstrap()
    const workspaceId = bootstrap.workspaces[0]!.id
    const listing = await backend.listDirectory({ workspaceId, path: '' }, CancellationToken.None)
    expect(listing.entries.map((entry) => entry.name)).toContain('src')
    const file = await backend.readFile({ workspaceId, path: 'README.md' }, CancellationToken.None)
    await backend.writeFile({
      workspaceId, path: 'README.md', content: '# Changed', expectedVersion: file.version,
    }, CancellationToken.None)
    await backend.gitStage({ workspaceId, path: 'README.md' }, CancellationToken.None)
    await expect(backend.gitStatus(workspaceId, CancellationToken.None)).resolves.toMatchObject({
      changes: [expect.objectContaining({ indexStatus: 'M' })],
    })
    backend.dispose()
  })

  it('publishes permission and question resolutions', async () => {
    const backend = new FakeDesktopBackend()
    const events: unknown[] = []
    backend.onEvent((event) => events.push(event))
    await backend.respondPermission({
      sessionId: 'session-desktop', runtimeId: 'runtime-desktop-1', generation: 1,
      agentId: 'worker', requestId: 'p', decision: 'allow_once',
    }, CancellationToken.None)
    await backend.respondQuestion({
      sessionId: 'session-protocol', runtimeId: 'runtime-protocol-1', generation: 1,
      agentId: 'worker', requestId: 'q', answers: ['yes'],
    }, CancellationToken.None)
    expect(events).toEqual([
      {
        type: 'permission_resolved', sessionId: 'session-desktop',
        runtimeId: 'runtime-desktop-1', generation: 1, agentId: 'worker', requestId: 'p',
      },
      {
        type: 'question_resolved', sessionId: 'session-protocol',
        runtimeId: 'runtime-protocol-1', generation: 1, agentId: 'worker', requestId: 'q',
      },
    ])
    await expect(backend.respondPermission({
      sessionId: 'session-desktop', runtimeId: 'runtime-desktop-1', generation: 1,
      agentId: 'worker', requestId: 'late', decision: 'deny',
    }, CancellationToken.Cancelled)).rejects.toThrow('Operation cancelled')
    backend.dispose()
  })
})
