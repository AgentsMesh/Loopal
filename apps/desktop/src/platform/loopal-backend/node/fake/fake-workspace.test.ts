import { CancellationToken } from '../../../../base/common/cancellation'
import { type DesktopEvent } from '../../../../shared/contracts'
import { FakeWorkspaceService } from './fake-workspace'

function fixture() {
  const events: DesktopEvent[] = []
  return {
    events,
    service: new FakeWorkspaceService('workspace', (event) => events.push(event)),
  }
}

describe('FakeWorkspaceService', () => {
  it('lists and reads its deterministic tree', async () => {
    const { service } = fixture()
    const root = await service.listDirectory({ workspaceId: 'workspace', path: '' }, CancellationToken.None)
    expect(root.entries.map((entry) => entry.name)).toEqual(['src', 'README.md'])
    const src = await service.listDirectory({
      workspaceId: 'workspace', path: 'src',
    }, CancellationToken.None)
    expect(src.entries.map((entry) => entry.name)).toEqual(['main.rs', 'workspace.rs'])
    await expect(service.readFile({
      workspaceId: 'workspace', path: 'README.md',
    }, CancellationToken.None)).resolves.toMatchObject({
      version: 'fake-1', languageId: 'markdown', readonly: false,
    })
    await expect(service.readFile({
      workspaceId: 'workspace', path: 'src',
    }, CancellationToken.None)).rejects.toThrow('File not found')
  })

  it('enforces CAS writes and publishes file and Git invalidations', async () => {
    const { service, events } = fixture()
    const updated = await service.writeFile({
      workspaceId: 'workspace', path: 'README.md', content: '# Updated',
      expectedVersion: 'fake-1',
    }, CancellationToken.None)
    expect(updated.version).toBe('fake-2')
    expect(events).toEqual([
      { type: 'file_changed', workspaceId: 'workspace', path: 'README.md', kind: 'changed' },
      { type: 'git_changed', workspaceId: 'workspace' },
    ])
    await expect(service.writeFile({
      workspaceId: 'workspace', path: 'README.md', content: 'stale',
      expectedVersion: 'fake-1',
    }, CancellationToken.None)).rejects.toThrow('FILE_VERSION_CONFLICT')
    await expect(service.writeFile({
      workspaceId: 'workspace', path: 'new.ts', content: 'export {}', expectedVersion: null,
    }, CancellationToken.None)).resolves.toMatchObject({ languageId: 'typescript' })
    const plain = await service.writeFile({
      workspaceId: 'workspace', path: 'notes.txt', content: 'plain', expectedVersion: null,
    }, CancellationToken.None)
    expect(plain.languageId).toBe('plaintext')
    await expect(service.gitStatus('workspace', CancellationToken.None)).resolves.toEqual(
      expect.objectContaining({ changes: expect.arrayContaining([
        expect.objectContaining({ path: 'new.ts', worktreeStatus: '?' }),
      ]) }),
    )
  })

  it('searches with glob and truncation and computes Git state', async () => {
    const { service } = fixture()
    await expect(service.search({
      workspaceId: 'workspace', query: 'loopal', glob: '*.md', maxResults: 10,
    }, CancellationToken.None)).resolves.toMatchObject({
      matches: [{ path: 'README.md', line: 1, column: 3 }], truncated: false,
    })
    await expect(service.search({
      workspaceId: 'workspace', query: 'loopal', maxResults: 1,
    }, CancellationToken.None)).resolves.toMatchObject({ truncated: true })
    await expect(service.search({
      workspaceId: 'workspace', query: 'Loopal', glob: '*',
    }, CancellationToken.None)).resolves.toMatchObject({ truncated: false })
    await service.writeFile({
      workspaceId: 'workspace', path: 'README.md', content: '# Changed', expectedVersion: 'fake-1',
    }, CancellationToken.None)
    await expect(service.gitStatus('workspace', CancellationToken.None)).resolves.toMatchObject({
      branch: 'main', changes: [{ path: 'README.md', worktreeStatus: 'M' }],
    })
    await expect(service.gitDiff({
      workspaceId: 'workspace', path: 'README.md',
    }, CancellationToken.None)).resolves.toMatchObject({ original: '# Loopal\n\nAgent workbench.\n' })
  })

  it('creates and safely removes worktrees', async () => {
    const { service } = fixture()
    await expect(service.listWorktrees('workspace', CancellationToken.None)).resolves.toHaveLength(2)
    await expect(service.createWorktree({
      workspaceId: 'workspace', name: 'feature',
    }, CancellationToken.None)).resolves.toMatchObject({ id: 'feature', isMain: false })
    await expect(service.createWorktree({
      workspaceId: 'workspace', name: 'feature',
    }, CancellationToken.None)).rejects.toThrow('WORKTREE_EXISTS')
    await expect(service.removeWorktree({
      workspaceId: 'workspace', name: 'review', force: false,
    }, CancellationToken.None)).rejects.toThrow('WORKTREE_DIRTY')
    await service.removeWorktree({
      workspaceId: 'workspace', name: 'review', force: true,
    }, CancellationToken.None)
    await expect(service.removeWorktree({
      workspaceId: 'workspace', name: 'missing', force: true,
    }, CancellationToken.None)).rejects.toThrow('WORKTREE_NOT_FOUND')
  })

  it('stages and unstages changed files', async () => {
    const { service, events } = fixture()
    await service.writeFile({
      workspaceId: 'workspace', path: 'README.md', content: '# Staged',
      expectedVersion: 'fake-1',
    }, CancellationToken.None)
    await service.gitStage({
      workspaceId: 'workspace', path: 'README.md',
    }, CancellationToken.None)
    await expect(service.gitStatus('workspace', CancellationToken.None)).resolves.toMatchObject({
      changes: [{ path: 'README.md', indexStatus: 'M', worktreeStatus: ' ' }],
    })
    await service.gitUnstage({
      workspaceId: 'workspace', path: 'README.md',
    }, CancellationToken.None)
    await expect(service.gitStatus('workspace', CancellationToken.None)).resolves.toMatchObject({
      changes: [{ path: 'README.md', indexStatus: ' ', worktreeStatus: 'M' }],
    })
    expect(events.filter((event) => event.type === 'git_changed')).toHaveLength(3)
    await expect(service.gitStage({
      workspaceId: 'workspace', path: 'src',
    }, CancellationToken.None)).rejects.toThrow('Git change not found')
  })

  it('rejects unknown workspaces and cancellation', async () => {
    const { service } = fixture()
    await expect(service.gitStatus('other', CancellationToken.None)).rejects.toThrow('Unknown workspace')
    await expect(service.listDirectory({
      workspaceId: 'workspace', path: '',
    }, CancellationToken.Cancelled)).rejects.toThrow('Operation cancelled')
  })
})
