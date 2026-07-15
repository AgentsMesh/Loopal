import { describe, expect, it } from 'vitest'
import { createWorkbenchRegistries } from './workbench-registries'

describe('workbench registries', () => {
  it('registers the built-in first-party panes explicitly', () => {
    const registries = createWorkbenchRegistries()
    expect(registries.panes.list('sidebar')).toEqual([])
    expect(registries.panes.list('editor').map((pane) => pane.id)).toEqual([
      'conversation',
      'federation',
    ])
    expect(registries.panes.get('explorer')).toBeUndefined()
    expect(registries.panes.get('search')).toBeUndefined()
    expect(registries.panes.get('source-control')).toBeUndefined()
    expect(registries.panes.get('diff')).toBeUndefined()
    expect(registries.panes.list('session').map((pane) => pane.id)).toEqual([
      'artifacts',
      'agents',
      'tasks',
      'diagnostics',
      'permissions',
      'questions',
    ])
    expect(registries.panes.list('panel')).toEqual([])
    expect(registries.panes.list('overlay').map((pane) => pane.id)).toEqual(['settings'])
    expect(registries.commands.list()).toEqual([])
    registries.contributions.dispose()
  })
})
