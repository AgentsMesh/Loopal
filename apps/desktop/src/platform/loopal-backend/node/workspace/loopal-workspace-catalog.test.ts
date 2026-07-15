import { pathToFileURL } from 'node:url'
import { type Workspace } from '../../../../shared/contracts'
import { LoopalWorkspaceCatalog } from './loopal-workspace-catalog'

const initial: Workspace = {
  id: 'local-workspace', name: 'Initial',
  rootUri: pathToFileURL('/initial').href, kind: 'folder',
}

describe('LoopalWorkspaceCatalog staging', () => {
  it('keeps pre-start workspaces invisible and shares a concurrent identity', () => {
    const catalog = new LoopalWorkspaceCatalog(initial)
    const failed = catalog.stage('/project', 'folder')
    const successful = catalog.stage('/project', 'folder')

    expect(failed.workspace.id).toBe(successful.workspace.id)
    expect(catalog.values()).toEqual([initial])
    successful.commit()

    expect(catalog.values()).toEqual([
      initial,
      expect.objectContaining({ id: successful.workspace.id, name: 'project' }),
    ])
  })

  it('reuses a workspace restored from a persisted Session location', () => {
    const catalog = new LoopalWorkspaceCatalog(initial)
    const restored = catalog.restore({
      sessionId: 'session-1', workspaceId: 'persisted-workspace',
      cwd: '/persisted', name: 'Persisted', kind: 'git_worktree',
    })

    const staged = catalog.stage('/persisted', 'folder')
    staged.commit()

    expect(staged.workspace).toBe(restored)
    expect(catalog.values()).toEqual([initial, restored])
  })
})
