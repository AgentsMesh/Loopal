import { type IDisposable, toDisposable } from '../../../base/common/lifecycle'

export type PaneKind =
  | 'conversation'
  | 'federation'
  | 'permissions'
  | 'questions'
  | 'plan'
  | 'tasks'
  | 'agents'
  | 'preview'
  | 'browser'
  | 'artifact'
  | 'diagnostics'
  | 'settings'

export interface PaneDescriptor {
  readonly id: string
  readonly kind: PaneKind
  readonly title: string
  readonly location: 'sidebar' | 'editor' | 'session' | 'panel' | 'overlay'
  readonly order: number
}

export class PaneRegistry {
  private readonly panes = new Map<string, PaneDescriptor>()

  register(descriptor: PaneDescriptor): IDisposable {
    if (this.panes.has(descriptor.id)) {
      throw new Error(`Pane already registered: ${descriptor.id}`)
    }
    this.panes.set(descriptor.id, descriptor)
    return toDisposable(() => {
      this.panes.delete(descriptor.id)
    })
  }

  get(id: string): PaneDescriptor | undefined {
    return this.panes.get(id)
  }

  list(location?: PaneDescriptor['location']): readonly PaneDescriptor[] {
    return [...this.panes.values()]
      .filter((pane) => location === undefined || pane.location === location)
      .sort((left, right) => left.order - right.order)
  }
}
