import { CommandRegistry } from './services/commands/command-registry'
import { ContributionRegistry } from './services/contributions/contribution-registry'
import { PaneRegistry } from './services/panes/pane-registry'

export interface WorkbenchRegistries {
  readonly commands: CommandRegistry
  readonly contributions: ContributionRegistry
  readonly panes: PaneRegistry
}

export function createWorkbenchRegistries(): WorkbenchRegistries {
  const registries: WorkbenchRegistries = {
    commands: new CommandRegistry(),
    contributions: new ContributionRegistry(),
    panes: new PaneRegistry(),
  }

  registries.panes.register({
    id: 'conversation',
    kind: 'conversation',
    title: 'Conversation',
    location: 'editor',
    order: 0,
  })
  registries.panes.register({
    id: 'federation',
    kind: 'federation',
    title: 'Federation',
    location: 'editor',
    order: 1,
  })
  registries.panes.register({
    id: 'artifacts',
    kind: 'artifact',
    title: 'Artifacts',
    location: 'session',
    order: 0,
  })
  registries.panes.register({
    id: 'agents',
    kind: 'agents',
    title: 'Agents',
    location: 'session',
    order: 1,
  })
  registries.panes.register({
    id: 'tasks',
    kind: 'tasks',
    title: 'Tasks',
    location: 'session',
    order: 2,
  })
  registries.panes.register({
    id: 'diagnostics',
    kind: 'diagnostics',
    title: 'Diagnostics',
    location: 'session',
    order: 3,
  })
  registries.panes.register({
    id: 'permissions',
    kind: 'permissions',
    title: 'Approvals',
    location: 'session',
    order: 4,
  })
  registries.panes.register({
    id: 'questions',
    kind: 'questions',
    title: 'Questions',
    location: 'session',
    order: 5,
  })
  registries.panes.register({
    id: 'settings',
    kind: 'settings',
    title: 'Settings',
    location: 'overlay',
    order: 0,
  })
  return registries
}
