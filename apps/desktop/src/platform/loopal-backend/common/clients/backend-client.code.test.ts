import { Event } from '../../../../base/common/event'
import { type ChannelClient } from '../../../ipc/common/channel'
import { DesktopBackendClient } from './backend-client'

type ChannelRequest = (
  channel: string,
  command: string,
  input?: unknown,
) => Promise<unknown>

class TestChannel implements ChannelClient {
  readonly request = vi.fn<ChannelRequest>()

  constructor(handler: ChannelRequest) {
    this.request.mockImplementation(handler)
  }

  async call<T>(channel: string, command: string, input?: unknown): Promise<T> {
    return await this.request(channel, command, input) as T
  }

  listen<T>(): Event<T> {
    return Event.none<T>()
  }

  dispose(): void {}
}

function channel(): TestChannel {
  return new TestChannel(async (_channel, command, input) => {
    const arg = input as Record<string, unknown>
    switch (command) {
      case 'listDirectory':
        return { workspaceId: arg.workspaceId, path: arg.path, entries: [] }
      case 'readFile':
      case 'writeFile':
        return {
          workspaceId: arg.workspaceId, path: arg.path, content: arg.content ?? 'text',
          version: 'v1', languageId: 'typescript', readonly: false,
        }
      case 'searchWorkspace': return { matches: [], truncated: false }
      case 'gitStatus': return { branch: 'main', ahead: 0, behind: 0, changes: [] }
      case 'gitDiff': return { path: arg.path, patch: '', original: '', modified: '' }
      case 'listWorktrees': return []
      case 'createWorktree':
        return {
          id: arg.name, path: `/tmp/${String(arg.name)}`, branch: 'branch', head: 'abc',
          isMain: false, hasChanges: false,
        }
      default: return undefined
    }
  })
}

describe('DesktopBackendClient code workbench façade', () => {
  it('maps every code workbench API onto a fixed channel command', async () => {
    const client = channel()
    const backend = new DesktopBackendClient(client)

    await expect(backend.listDirectory({ workspaceId: 'w', path: '' }))
      .resolves.toMatchObject({ workspaceId: 'w' })
    await expect(backend.readFile({ workspaceId: 'w', path: 'a.ts' }))
      .resolves.toMatchObject({ content: 'text' })
    await expect(backend.writeFile({
      workspaceId: 'w', path: 'a.ts', content: 'next', expectedVersion: 'v0',
    })).resolves.toMatchObject({ content: 'next' })
    await expect(backend.searchWorkspace({ workspaceId: 'w', query: 'next' }))
      .resolves.toEqual({ matches: [], truncated: false })
    await expect(backend.gitStatus('w')).resolves.toMatchObject({ branch: 'main' })
    await expect(backend.gitDiff({ workspaceId: 'w', path: 'a.ts' }))
      .resolves.toMatchObject({ path: 'a.ts' })
    await backend.gitStage({ workspaceId: 'w', path: 'a.ts' })
    await backend.gitUnstage({ workspaceId: 'w', path: 'a.ts' })
    await expect(backend.listWorktrees('w')).resolves.toEqual([])
    await expect(backend.createWorktree({ workspaceId: 'w', name: 'feature' }))
      .resolves.toMatchObject({ id: 'feature' })
    await backend.removeWorktree({ workspaceId: 'w', name: 'feature', force: false })
    await backend.respondPermission({
      sessionId: 's', runtimeId: 'r', generation: 1, agentId: 'main',
      requestId: 'p', decision: 'deny',
    })
    await backend.respondQuestion({
      sessionId: 's', runtimeId: 'r', generation: 1, agentId: 'main',
      requestId: 'q', answers: ['yes'],
    })
    await backend.respondQuestion({
      sessionId: 's', runtimeId: 'r', generation: 1, agentId: 'main',
      requestId: 'cancel', cancelled: true,
    })

    expect(client.request).toHaveBeenCalledWith('desktopBackend', 'gitStage', {
      workspaceId: 'w', path: 'a.ts',
    })
    expect(client.request).toHaveBeenCalledWith('desktopBackend', 'respondQuestion', {
      sessionId: 's', runtimeId: 'r', generation: 1, agentId: 'main',
      requestId: 'cancel', cancelled: true,
    })
    expect(client.request).toHaveBeenCalledWith('desktopBackend', 'respondQuestion', {
      sessionId: 's', runtimeId: 'r', generation: 1, agentId: 'main',
      requestId: 'q', answers: ['yes'],
    })
  })

  it('rejects malformed code workbench responses', async () => {
    const client = channel()
    client.request.mockResolvedValue({ invalid: true })
    const backend = new DesktopBackendClient(client)
    await expect(backend.readFile({ workspaceId: 'w', path: 'a.ts' })).rejects.toThrow()
    await expect(backend.gitStatus('w')).rejects.toThrow()
  })
})
