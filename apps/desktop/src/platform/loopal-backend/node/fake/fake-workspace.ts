import { basename, dirname, extname } from 'node:path/posix'
import { type CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  type CreateWorktreeInput,
  type DesktopEvent,
  type GitStageInput,
  type GitUnstageInput,
  type ListDirectoryInput,
  type ReadFileInput,
  type RemoveWorktreeInput,
  type WorkspaceSearchInput,
  type WriteFileInput,
  type Worktree,
} from '../../../../shared/contracts'

interface FakeNode {
  kind: 'file' | 'directory'
  content: string
  version: number
  original: string
  staged: boolean
}

export class FakeWorkspaceService {
  private readonly nodes = new Map<string, FakeNode>([
    ['README.md', file('# Loopal\n\nAgent workbench.\n')],
    ['src', directory()],
    ['src/main.rs', file('fn main() {\n    println!("loopal");\n}\n')],
    ['src/workspace.rs', file('pub struct Workspace;\n')],
  ])
  private readonly worktrees: Worktree[] = [{
    id: 'main', path: '/workspace/loopal', branch: 'main', head: '87ad2b93',
    isMain: true, hasChanges: false,
  }, {
    id: 'review', path: '/workspace/loopal/.loopal/worktrees/review', branch: 'loopal-wt-review',
    head: '87ad2b93', isMain: false, hasChanges: true,
  }]

  constructor(
    private readonly workspaceId: string,
    private readonly emit: (event: DesktopEvent) => void,
  ) {}

  async listDirectory(input: ListDirectoryInput, token: CancellationToken) {
    this.check(input.workspaceId, token)
    const entries = [...this.nodes.entries()]
      .filter(([path]) => dirname(path) === (input.path || '.'))
      .map(([path, node]) => ({
        path,
        name: basename(path),
        kind: node.kind,
        size: new TextEncoder().encode(node.content).length,
      }))
      .sort((left, right) => kindRank(left.kind) - kindRank(right.kind)
        || left.name.localeCompare(right.name))
    return { workspaceId: this.workspaceId, path: input.path, entries }
  }

  async readFile(input: ReadFileInput, token: CancellationToken) {
    this.check(input.workspaceId, token)
    const node = this.nodes.get(input.path)
    if (!node || node.kind !== 'file') throw new Error(`File not found: ${input.path}`)
    return document(this.workspaceId, input.path, node)
  }

  async writeFile(input: WriteFileInput, token: CancellationToken) {
    this.check(input.workspaceId, token)
    const current = this.nodes.get(input.path)
    const version = current ? `fake-${current.version}` : null
    if (version !== input.expectedVersion) throw new Error('FILE_VERSION_CONFLICT')
    const node: FakeNode = {
      kind: 'file', content: input.content, version: (current?.version ?? 0) + 1,
      original: current?.original ?? '', staged: current?.staged ?? false,
    }
    this.nodes.set(input.path, node)
    this.emit({
      type: 'file_changed', workspaceId: this.workspaceId, path: input.path,
      kind: current ? 'changed' : 'created',
    })
    this.emit({ type: 'git_changed', workspaceId: this.workspaceId })
    return document(this.workspaceId, input.path, node)
  }

  async search(input: WorkspaceSearchInput, token: CancellationToken) {
    this.check(input.workspaceId, token)
    const limit = input.maxResults ?? 200
    const matches = []
    for (const [path, node] of this.nodes) {
      if (node.kind !== 'file' || (input.glob && !globMatches(path, input.glob))) continue
      for (const [index, text] of node.content.split('\n').entries()) {
        const column = text.toLocaleLowerCase().indexOf(input.query.toLocaleLowerCase())
        if (column < 0) continue
        matches.push({ path, line: index + 1, column: column + 1, preview: text })
        if (matches.length === limit) return { matches, truncated: true }
      }
    }
    return { matches, truncated: false }
  }

  async gitStatus(workspaceId: string, token: CancellationToken) {
    this.check(workspaceId, token)
    const changes = [...this.nodes.entries()]
      .filter(([, node]) => node.kind === 'file' && node.content !== node.original)
      .map(([path, node]) => ({
        path,
        indexStatus: node.staged ? (node.original ? 'M' : 'A') : ' ',
        worktreeStatus: node.staged ? ' ' : node.original ? 'M' : '?',
      }))
    return { branch: 'main', ahead: 0, behind: 0, changes }
  }

  async gitDiff(input: ReadFileInput, token: CancellationToken) {
    const current = await this.readFile(input, token)
    const original = this.nodes.get(input.path)!.original
    return {
      path: input.path, original, modified: current.content,
      patch: `--- a/${input.path}\n+++ b/${input.path}\n-${original}\n+${current.content}`,
    }
  }

  async gitStage(input: GitStageInput, token: CancellationToken) {
    this.change(input.workspaceId, input.path, token).staged = true
    this.emit({ type: 'git_changed', workspaceId: this.workspaceId })
  }

  async gitUnstage(input: GitUnstageInput, token: CancellationToken) {
    this.change(input.workspaceId, input.path, token).staged = false
    this.emit({ type: 'git_changed', workspaceId: this.workspaceId })
  }

  async listWorktrees(workspaceId: string, token: CancellationToken) {
    this.check(workspaceId, token)
    return structuredClone(this.worktrees)
  }

  async createWorktree(input: CreateWorktreeInput, token: CancellationToken) {
    this.check(input.workspaceId, token)
    if (this.worktrees.some((item) => item.id === input.name)) throw new Error('WORKTREE_EXISTS')
    const worktree: Worktree = {
      id: input.name, path: `/workspace/loopal/.loopal/worktrees/${input.name}`,
      branch: `loopal-wt-${input.name}`, head: '87ad2b93', isMain: false, hasChanges: false,
    }
    this.worktrees.push(worktree)
    return structuredClone(worktree)
  }

  async removeWorktree(input: RemoveWorktreeInput, token: CancellationToken) {
    this.check(input.workspaceId, token)
    const index = this.worktrees.findIndex((item) => item.id === input.name && !item.isMain)
    const worktree = this.worktrees[index]
    if (!worktree) throw new Error('WORKTREE_NOT_FOUND')
    if (worktree.hasChanges && !input.force) throw new Error('WORKTREE_DIRTY')
    this.worktrees.splice(index, 1)
  }

  private check(workspaceId: string, token: CancellationToken): void {
    throwIfCancelled(token)
    if (workspaceId !== this.workspaceId) throw new Error(`Unknown workspace: ${workspaceId}`)
  }

  private change(workspaceId: string, path: string, token: CancellationToken): FakeNode {
    this.check(workspaceId, token)
    const node = this.nodes.get(path)
    if (!node || node.kind !== 'file' || node.content === node.original) {
      throw new Error(`Git change not found: ${path}`)
    }
    return node
  }
}

function file(content: string): FakeNode {
  return { kind: 'file', content, original: content, version: 1, staged: false }
}
function directory(): FakeNode {
  return { kind: 'directory', content: '', original: '', version: 1, staged: false }
}
function document(workspaceId: string, path: string, node: FakeNode) {
  const languages: Record<string, string> = { '.md': 'markdown', '.rs': 'rust', '.ts': 'typescript' }
  return {
    workspaceId, path, content: node.content, version: `fake-${node.version}`,
    languageId: languages[extname(path)] ?? 'plaintext', readonly: false,
  }
}
function globMatches(path: string, glob: string): boolean {
  return glob === '*' || path.endsWith(glob.replace(/^\*+/, ''))
}
function kindRank(kind: 'file' | 'directory' | 'symlink'): number {
  return kind === 'directory' ? 0 : kind === 'file' ? 1 : 2
}
