import {
  CreateWorktreeInputSchema,
  DirectoryListingSchema,
  FileDocumentSchema,
  GitDiffSchema,
  GitStageInputSchema,
  GitStatusSchema,
  RelativePathSchema,
  RemoveWorktreeInputSchema,
  GitUnstageInputSchema,
  WorkspaceDesktopEventSchema,
  WorkspaceSearchInputSchema,
  WorkspaceSearchResultSchema,
  WorktreeListSchema,
  WriteFileInputSchema,
} from './workspace-contracts'

describe('workspace contracts', () => {
  it('accepts normalized relative paths and rejects escape forms', () => {
    expect(RelativePathSchema.parse('')).toBe('')
    expect(RelativePathSchema.parse('src/main.rs')).toBe('src/main.rs')
    expect(() => RelativePathSchema.parse('/etc/passwd')).toThrow()
    expect(() => RelativePathSchema.parse('../secret')).toThrow()
    expect(() => RelativePathSchema.parse('src\\..\\secret')).toThrow()
  })

  it('validates files, listings, CAS writes, and search defaults', () => {
    expect(DirectoryListingSchema.parse({
      workspaceId: 'w', path: '', entries: [{ path: 'src', name: 'src', kind: 'directory', size: 0 }],
    }).entries).toHaveLength(1)
    expect(FileDocumentSchema.parse({
      workspaceId: 'w', path: 'a.ts', content: '', version: 'v1',
      languageId: 'typescript', readonly: false,
    }).version).toBe('v1')
    expect(WriteFileInputSchema.parse({
      workspaceId: 'w', path: 'a.ts', content: 'x', expectedVersion: null,
    }).expectedVersion).toBeNull()
    expect(WorkspaceSearchInputSchema.parse({ workspaceId: 'w', query: 'needle' }).maxResults)
      .toBe(200)
    expect(WorkspaceSearchResultSchema.parse({
      matches: [{ path: 'a.ts', line: 1, column: 2, preview: 'needle' }], truncated: false,
    }).matches[0]?.line).toBe(1)
  })

  it('validates Git, worktree, and workspace events', () => {
    expect(GitStatusSchema.parse({
      branch: null, ahead: 1, behind: 2,
      changes: [{ path: 'a.ts', indexStatus: 'M', worktreeStatus: ' ' }],
    }).ahead).toBe(1)
    expect(GitDiffSchema.parse({
      path: 'a.ts', patch: '@@', original: 'a', modified: 'b',
    }).modified).toBe('b')
    expect(GitStageInputSchema.parse({ workspaceId: 'w', path: 'a.ts' }).path).toBe('a.ts')
    expect(GitUnstageInputSchema.parse({ workspaceId: 'w', path: 'a.ts' }).path).toBe('a.ts')
    expect(WorktreeListSchema.parse([{
      id: 'main', path: '/tmp/repo', branch: 'main', head: 'abc',
      isMain: true, hasChanges: false,
    }])).toHaveLength(1)
    expect(CreateWorktreeInputSchema.parse({ workspaceId: 'w', name: 'feature_1' }).name)
      .toBe('feature_1')
    expect(RemoveWorktreeInputSchema.parse({
      workspaceId: 'w', name: 'feature', force: true,
    }).force).toBe(true)
    expect(() => CreateWorktreeInputSchema.parse({ workspaceId: 'w', name: '../escape' })).toThrow()
    expect(WorkspaceDesktopEventSchema.parse({
      type: 'file_changed', workspaceId: 'w', path: 'a.ts', kind: 'changed',
    }).type).toBe('file_changed')
    expect(WorkspaceDesktopEventSchema.parse({ type: 'git_changed', workspaceId: 'w' }).type)
      .toBe('git_changed')
    const resync = WorkspaceDesktopEventSchema.parse({
      type: 'workspace_resync_required', workspaceId: 'w', reason: 'overflow',
    })
    expect(resync.type === 'workspace_resync_required' && resync.reason).toBe('overflow')
  })
})
