import {
  type AgentControlCommand, type HostStatus, type SessionDetail,
} from '../../../../shared/contracts'
import { AgentTopology } from '../../agents/browser/agent-topology'
import { ArtifactPanel } from './artifact-panel'
import { DiagnosticsInspector } from './diagnostics-inspector'
import { McpRuntimePanel } from './mcp-runtime-panel'
import {
  type SessionPanelId, type SessionPanelState,
} from './session-panel-state'
import { TaskInspector } from './task-inspector'
import { WorkflowInspector } from './workflow-inspector'

export function SessionPanelContent(props: {
  readonly panelId: SessionPanelId
  readonly detail?: SessionDetail
  readonly hostStatus: HostStatus
  readonly state: SessionPanelState
  readonly canControl: boolean
  readonly busy: boolean
  readonly onControl: (command: AgentControlCommand) => void
  readonly onSelectAgent: (agentId: string) => void
}): React.JSX.Element {
  const view = props.state.view
  switch (props.panelId) {
    case 'agents':
      return <AgentTopology agents={props.state.localAgents}
        selectedAgentId={props.state.selected?.id} onSelect={props.onSelectAgent} />
    case 'tasks':
      return <TaskInspector key={props.state.selected?.id} view={view}
        sections={['goal', 'tasks']} showEmpty={false} />
    case 'workflows':
      return <WorkflowInspector view={view} />
    case 'background':
      return <TaskInspector view={view} canControl={props.canControl}
        busy={props.busy} onControl={props.onControl} sections={['background']}
        testId="background-tasks-pane" showEmpty={false} />
    case 'scheduled':
      return <TaskInspector view={view} canControl={props.canControl}
        busy={props.busy} onControl={props.onControl} sections={['crons']}
        testId="scheduled-work-pane" showEmpty={false} />
    case 'artifacts':
      return <ArtifactPanel artifacts={props.detail?.artifacts ?? []} />
    case 'mcp':
      return <McpRuntimePanel view={view!} canControl={props.canControl}
        busy={props.busy} onControl={props.onControl} />
    case 'diagnostics':
      return <DiagnosticsInspector hostStatus={props.hostStatus} detail={props.detail}
        agentId={props.state.selected?.id} canControl={props.canControl}
        busy={props.busy} onControl={props.onControl} />
  }
}
