import { createHash } from 'node:crypto'
import { basename } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { type Workspace } from '../../../../shared/contracts'
import { type SessionLocation } from '../sessions/loopal-session-resume-state'

export interface WorkspaceCatalogStage {
  readonly workspace: Workspace
  commit(): void
}

export class LoopalWorkspaceCatalog {
  private readonly entries = new Map<string, Workspace>()

  constructor(readonly initial: Workspace) {
    this.entries.set(initial.id, initial)
  }

  ensure(path: string, kind: Workspace['kind'], name = basename(path) || 'Workspace'): Workspace {
    const stage = this.stage(path, kind, name)
    stage.commit()
    return stage.workspace
  }

  stage(
    path: string, kind: Workspace['kind'], name = basename(path) || 'Workspace',
  ): WorkspaceCatalogStage {
    const rootUri = pathToFileURL(path).href
    const existing = [...this.entries.values()].find((value) => value.rootUri === rootUri)
    if (existing) return { workspace: existing, commit: () => undefined }
    const id = `local-${createHash('sha256').update(rootUri).digest('hex').slice(0, 16)}`
    const workspace = { id, name, rootUri, kind }
    return { workspace, commit: () => this.entries.set(id, workspace) }
  }

  restore(location: SessionLocation): Workspace {
    const workspace: Workspace = {
      id: location.workspaceId,
      name: location.name,
      rootUri: pathToFileURL(location.cwd).href,
      kind: location.kind,
    }
    this.entries.set(workspace.id, workspace)
    return workspace
  }

  get(id: string): Workspace | undefined { return this.entries.get(id) }
  path(id: string): string | undefined {
    const workspace = this.get(id)
    return workspace ? fileURLToPath(workspace.rootUri) : undefined
  }
  values(): readonly Workspace[] { return [...this.entries.values()] }
}
