import { SessionNavigator } from '../contrib/sessions/browser/session-navigator'
import { type useWorkbenchController } from './use-workbench-controller'
import { type useFederationController } from '../contrib/federation/browser/use-federation-controller'

export function WorkbenchSidebar(props: {
  readonly controller: ReturnType<typeof useWorkbenchController>
  readonly federation: ReturnType<typeof useFederationController>
  readonly onRequestCreate: () => void
}): React.JSX.Element {
  const controller = props.controller
  return <SessionNavigator currentSessions={controller.currentSessions}
    searchResults={controller.searchResults}
    {...(controller.activeSessionId !== undefined
      ? { activeSessionId: controller.activeSessionId }
      : {})}
    query={controller.query} searchRef={controller.searchRef}
    canCreate={controller.canCreate} onQueryChange={controller.setQuery}
    onOpenSession={controller.openSession} onRequestCreate={props.onRequestCreate}
    federation={{ memberships: props.federation.snapshot.memberships,
      busy: props.federation.busy, onJoin: props.federation.join,
      onLeave: props.federation.leave }} />
}
