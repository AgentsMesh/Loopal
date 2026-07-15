export enum ServiceScope {
  App = 'app',
  Window = 'window',
  Workspace = 'workspace',
  Pane = 'pane',
}

export interface ScopeDescriptor {
  readonly id: string
  readonly scope: ServiceScope
}
