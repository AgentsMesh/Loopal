import { DisposableStore, type IDisposable, toDisposable } from '../../../base/common/lifecycle'

export interface WorkbenchContributionContext {
  readonly disposables: DisposableStore
}

export interface WorkbenchContribution {
  readonly id: string
  activate(context: WorkbenchContributionContext): void
}

export class ContributionRegistry implements IDisposable {
  private readonly contributions = new Map<string, WorkbenchContribution>()
  private readonly active = new Map<string, DisposableStore>()

  register(contribution: WorkbenchContribution): IDisposable {
    if (this.contributions.has(contribution.id)) {
      throw new Error(`Contribution already registered: ${contribution.id}`)
    }
    this.contributions.set(contribution.id, contribution)
    return toDisposable(() => {
      this.deactivate(contribution.id)
      this.contributions.delete(contribution.id)
    })
  }

  activateAll(): void {
    for (const contribution of this.contributions.values()) {
      if (this.active.has(contribution.id)) {
        continue
      }
      const disposables = new DisposableStore()
      contribution.activate({ disposables })
      this.active.set(contribution.id, disposables)
    }
  }

  dispose(): void {
    for (const id of [...this.active.keys()]) {
      this.deactivate(id)
    }
    this.contributions.clear()
  }

  private deactivate(id: string): void {
    this.active.get(id)?.dispose()
    this.active.delete(id)
  }
}
