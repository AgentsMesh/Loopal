import { useEffect, useReducer, useRef, useState } from 'react'
import { type LoopalDesktopAPI } from '../../shared/contracts'
import { ActivityBar } from './activity-bar'
import { useDesktopPreferences } from '../contrib/settings/browser/desktop-preferences'
import { openFederationConversation } from '../contrib/federation/browser/federation-conversation'
import { SessionAttention } from '../contrib/attention/browser/session-attention'
import { SessionCreateDialog } from '../contrib/sessions/browser/session-create-dialog'
import { SessionPanelZone } from '../contrib/session-panels/browser/session-panel-zone'
import { SessionWorkspace } from './session-workspace'
import { type Stage2WorkbenchBinding } from './stage2-view-model'
import { StatusBar } from './status-bar'
import { useFederationController } from '../contrib/federation/browser/use-federation-controller'
import { useWorkbenchController } from './use-workbench-controller'
import { useWorkbenchRuntimeController } from './use-workbench-runtime-controller'
import { useAgentControl } from '../contrib/agents/browser/use-agent-control'
import { useSlashCommands } from '../contrib/conversation/browser/use-slash-commands'
import { useWorkbenchShortcuts } from './use-workbench-shortcuts'
import { WorkbenchSettingsOverlay } from '../contrib/settings/browser/workbench-settings-overlay'
import { WorkbenchSidebar } from './workbench-sidebar'
import { buildControllerContext } from './workbench-context'
import {
  createWorkbenchViewState, reduceWorkbenchView, type WorkbenchArea,
} from './workbench-view-state'
interface WorkbenchProps { readonly api: LoopalDesktopAPI; readonly stage2?: Stage2WorkbenchBinding }
export function Workbench({ api, stage2 }: WorkbenchProps): React.JSX.Element {
  const controller = useWorkbenchController(api)
  const federation = useFederationController(
    api, controller.projection.sessions, controller.projection.runtimes,
  )
  const agentControl = useAgentControl(api, controller.projection)
  const slash = useSlashCommands(api, controller.activeWorkspaceId)
  const fallbackContext = buildControllerContext(controller)
  const runtime = useWorkbenchRuntimeController(api, fallbackContext, stage2 === undefined)
  const binding = stage2 ?? runtime
  const model = binding.model
  const callbacks = binding.callbacks
  const activeError = controller.error
  const [viewState, dispatchView] = useReducer(
    reduceWorkbenchView,
    createWorkbenchViewState(),
  )
  const [selectedAgentId, setSelectedAgentId] = useState('main')
  const [creatingSession, setCreatingSession] = useState(false)
  const [preferences, updatePreferences] = useDesktopPreferences()
  const previousSessionId = useRef<string | undefined>(undefined)
  useEffect(() => {
    if (previousSessionId.current !== undefined
      && previousSessionId.current !== controller.activeSessionId) {
      setSelectedAgentId('main')
      dispatchView({ type: 'close_settings' })
    }
    previousSessionId.current = controller.activeSessionId
  }, [controller.activeSessionId])
  const selectedAgent = controller.projection.detail?.agents.find(
    (agent) => agent.id === selectedAgentId,
  ) ?? controller.projection.detail?.agents.find((agent) => agent.id === 'main')
    ?? controller.projection.detail?.agents[0]
  const activeAgentId = selectedAgent?.id ?? selectedAgentId
  const submit = async (agentId = activeAgentId): Promise<void> => {
    const result = await slash.execute(
      controller.draftFor(agentId), agentId, agentControl.control, controller.images.length > 0,
    )
    if (result === 'message') await controller.send(agentId)
    else if (result === 'handled') controller.setDraftFor(agentId, '')
  }
  const executeCommand = async (command: string, agentId: string): Promise<void> => {
    const result = await slash.execute(
      command, agentId, agentControl.control, controller.images.length > 0,
    )
    if (result === 'handled') controller.setDraftFor(agentId, '')
  }
  const canControlAgent = selectedAgent !== undefined
    && agentControl.available(activeAgentId)
  const activateArea = (area: WorkbenchArea): void => {
    dispatchView({ type: 'select_area', area })
  }
  const selectAgent = (agentId: string): void => {
    setSelectedAgentId(agentId)
  }
  const showSessions = (): void => activateArea('conversation')
  const openAttention = (): void => {
    dispatchView({ type: 'close_settings' })
    showSessions()
    requestAnimationFrame(() => document.querySelector('[data-testid="session-attention"]')
      ?.scrollIntoView({ block: 'nearest' }))
  }
  useWorkbenchShortcuts({
    selectArea: activateArea,
    toggleSidebar: () => dispatchView({ type: 'toggle_sidebar' }),
    toggleSettings: () => dispatchView({ type: 'toggle_settings' }),
  })
  return (
    <div
      className={`workbench density-${preferences.panelDensity} ${
        !viewState.sidebarVisible || viewState.area === 'federation' ? 'sidebar-collapsed' : ''
      }`}
      data-testid="workbench"
      style={{ '--conversation-font-size': `${preferences.conversationFontSize}px` } as React.CSSProperties}
    >
      <ActivityBar
        activeArea={viewState.area}
        sidebarVisible={viewState.sidebarVisible}
        attentionCount={model.permissions.length + model.questions.length + model.planApprovals.length}
        onActivate={activateArea}
        onToggleSidebar={() => dispatchView({ type: 'toggle_sidebar' })}
        onOpenAttention={openAttention}
        settingsOpen={viewState.settingsOpen}
        onOpenSettings={() => dispatchView({ type: 'toggle_settings' })}
      />
      {viewState.sidebarVisible && viewState.area !== 'federation' && (
        <WorkbenchSidebar controller={controller} federation={federation}
          onRequestCreate={() => setCreatingSession(true)} />
      )}
      <section className="workbench-center"
        data-testid="primary-workspace" data-workspace={viewState.area}>
        {model.error && <div className="stage2-error" role="alert">{model.error}</div>}
        {agentControl.error && (
          <div className="stage2-error" role="alert">{agentControl.error}</div>
        )}
        {federation.error && viewState.area !== 'federation' && (
          <div className="stage2-error" role="alert">{federation.error}</div>
        )}
        <SessionWorkspace
          {...(controller.projection.detail !== undefined
            ? { detail: controller.projection.detail }
            : {})}
          {...(activeError !== undefined ? { error: activeError } : {})}
          draft={controller.draftFor(activeAgentId)} sending={controller.sending}
          images={controller.images}
          onDraftChange={(value) => {
            slash.clearFeedback()
            controller.setDraftFor(activeAgentId, value)
          }}
          onSelectImages={controller.selectImages}
          onRemoveImage={controller.removeImage}
          onSend={submit}
          onStop={controller.stopSession}
          onRestart={controller.restartSession}
          lifecycleBusy={controller.lifecycleBusy}
          selectedAgentId={activeAgentId}
          controlBusy={agentControl.busy}
          controlAvailable={agentControl.available}
          onControl={agentControl.control}
          commands={slash.items}
          {...(slash.error ? { commandError: slash.error } : {})}
          {...(slash.helpQuery !== undefined ? { commandHelpQuery: slash.helpQuery } : {})}
          onRequestCommands={slash.refresh}
          onDismissCommandHelp={slash.dismissHelp}
          onExecuteCommand={(command, agentId) => void executeCommand(command, agentId)}
          surface={viewState.area === 'federation' ? 'federation' : 'conversation'}
          federation={federation}
          onOpenAgentConversation={(target) => void openFederationConversation(
            target, federation.snapshot, controller.openSession, selectAgent, showSessions,
            (sessionId) => { previousSessionId.current = sessionId },
          )}
          onOpenSettings={() => dispatchView({ type: 'open_settings' })}
          panel={(
            <SessionPanelZone
              {...(controller.projection.detail !== undefined
                ? { detail: controller.projection.detail }
                : {})}
              hostStatus={controller.projection.hostStatus}
              selectedAgentId={activeAgentId}
              onSelectAgent={selectAgent}
              canControl={canControlAgent}
              busy={agentControl.busy}
              onControl={(command) => void agentControl.control(activeAgentId, command)}
              showTopology={preferences.showAgentTopology}
            />
          )}
          attention={<SessionAttention model={model} callbacks={callbacks} />}
        />
        {viewState.settingsOpen && <WorkbenchSettingsOverlay api={api}
          controller={controller} agentControl={agentControl} activeAgentId={activeAgentId}
          canControlAgent={canControlAgent} preferences={preferences}
          onPreferences={updatePreferences} onSelectAgent={selectAgent}
          onClose={() => dispatchView({ type: 'close_settings' })} />}
      </section>
      <StatusBar sessions={controller.projection.sessions}
        federation={federation.snapshot}
        onOpenFederation={() => activateArea('federation')} />
      {creatingSession && <SessionCreateDialog api={api}
        onCreate={controller.createSession} onClose={() => setCreatingSession(false)} />}
    </div>
  )
}
