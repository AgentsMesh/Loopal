import {
  type AgentControlCommand,
  type DesktopImageAttachment,
  type SessionDetail,
} from '../../shared/contracts'
import { canRestartSession, isSessionLive } from '../../shared/contracts/session-lifecycle'
import { ConversationView } from '../contrib/conversation/browser/conversation-view'
import { type FederationConversationTarget } from '../contrib/federation/browser/federation-model'
import { FederationWorkspace } from '../contrib/federation/browser/federation-workspace'
import { SessionRuntimeStatus } from '../contrib/sessions/browser/session-runtime-status'
import { MessageComposer } from '../contrib/conversation/browser/message-composer'
import { SessionToolbar } from '../contrib/sessions/browser/session-toolbar'
import { type SlashCommandItem } from '../contrib/conversation/browser/slash-command-model'
import { useI18n } from './i18n-context'
import { type useFederationController } from '../contrib/federation/browser/use-federation-controller'

interface SessionWorkspaceProps {
  readonly detail?: SessionDetail
  readonly error?: string
  readonly draft: string
  readonly sending: boolean
  readonly images: readonly DesktopImageAttachment[]
  readonly onDraftChange: (draft: string) => void
  readonly onSelectImages: () => Promise<void>
  readonly onRemoveImage: (index: number) => void
  readonly onSend: (agentId?: string) => Promise<void>
  readonly onStop: () => Promise<void>
  readonly onRestart: () => Promise<void>
  readonly lifecycleBusy: boolean
  readonly selectedAgentId: string
  readonly controlBusy: boolean
  readonly controlAvailable: (agentId: string) => boolean
  readonly onControl: (agentId: string, command: AgentControlCommand) => Promise<boolean>
  readonly commands: readonly SlashCommandItem[]
  readonly commandError?: string
  readonly commandHelpQuery?: string
  readonly onRequestCommands: () => void
  readonly onDismissCommandHelp: () => void
  readonly onExecuteCommand: (command: string, agentId: string) => void
  readonly surface: 'conversation' | 'federation'
  readonly onOpenAgentConversation: (target: FederationConversationTarget) => void
  readonly onOpenSettings: () => void
  readonly federation: ReturnType<typeof useFederationController>
  readonly panel?: React.ReactNode
  readonly attention?: React.ReactNode
}

export function SessionWorkspace(props: SessionWorkspaceProps): React.JSX.Element {
  const { t } = useI18n()
  const selectedAgent = props.detail?.agents.find((agent) => agent.id === props.selectedAgentId)
    ?? props.detail?.agents.find((agent) => agent.id === 'main')
    ?? props.detail?.agents[0]
  const isRootAgent = selectedAgent !== undefined && !selectedAgent.parentId
  const isRootConversation = selectedAgent === undefined || isRootAgent
  const conversation = selectedAgent && !isRootAgent
    ? selectedAgent.conversation ?? []
    : props.detail?.conversation ?? []
  const conversationView = selectedAgent?.view
    ?? (isRootConversation ? props.detail?.view : undefined)
  const controlAvailable = selectedAgent !== undefined
    && props.controlAvailable(selectedAgent.id)
  const canControl = controlAvailable && !props.controlBusy
  const mode = selectedAgent?.mode ?? props.detail?.session.mode
  const agentCanReceive = selectedAgent === undefined
    || (selectedAgent.controllable !== false
      && (['starting', 'idle', 'running', 'waiting', 'suspended'].includes(selectedAgent.status)
        || (isRootAgent && selectedAgent.status === 'failed')))
  const retiredAgent = selectedAgent?.status === 'completed' || selectedAgent?.status === 'failed'
  const sessionLive = Boolean(props.detail && isSessionLive(props.detail.session))
  const composeDisabled = !props.detail || props.sending
    || !sessionLive || !agentCanReceive
  const composerPlaceholder = !agentCanReceive
    ? retiredAgent
      ? t('composer.retired', {
          name: selectedAgent?.name ?? 'Agent', status: selectedAgent?.status ?? '',
        })
      : t('composer.notReady', { name: selectedAgent?.name ?? 'Agent' })
    : t('composer.placeholder', { name: selectedAgent?.name ?? 'Loopal' })
  const control = (command: AgentControlCommand): void => {
    if (selectedAgent) void props.onControl(selectedAgent.id, command)
  }
  return (
    <main className="session-workspace">
      {props.surface === 'federation' ? (
        <FederationWorkspace snapshot={props.federation.snapshot}
          {...(props.federation.busy ? { busy: props.federation.busy } : {})}
          {...(props.federation.error ? { error: props.federation.error } : {})}
          onStart={props.federation.start} onRefresh={props.federation.refresh}
          onOpenConversation={props.onOpenAgentConversation}
          onManage={props.onOpenSettings} />
      ) : (
        <>
          <SessionToolbar {...(props.detail ? { detail: props.detail } : {})}
            {...(selectedAgent ? { selectedAgent } : {})} />
          <section className="conversation" aria-label={t('workspace.conversation')} data-testid="conversation">
            {selectedAgent && !isRootAgent && (
              <div className="agent-conversation-heading">
                {t('workspace.viewingAgent', {
                  name: selectedAgent.name, status: selectedAgent.status,
                })}
                {selectedAgent.error ? ` · ${selectedAgent.error}` : ''}
              </div>
            )}
            {props.detail && (
              <ConversationView
                key={selectedAgent?.id ?? 'main'}
                entries={conversation}
                {...(conversationView ? { view: conversationView } : {})}
              />
            )}
            {!props.detail && !props.error && (
              <p className="loading">{t('workspace.connecting')}</p>
            )}
            {props.error && <div className="error-banner" role="alert">{props.error}</div>}
          </section>
          {props.panel}
          {props.attention}
          <MessageComposer
            label={isRootConversation
              ? t('workspace.messageLoopal')
              : t('workspace.messageAgent', { name: selectedAgent?.name ?? 'Agent' })}
            placeholder={composerPlaceholder}
            draft={props.draft}
            images={props.images}
            disabled={composeDisabled}
            sending={props.sending}
            canControl={canControl}
            hasSession={Boolean(props.detail)}
            sessionLive={sessionLive}
            canRestartSession={Boolean(props.detail
              && canRestartSession(props.detail.session))}
            lifecycleBusy={props.lifecycleBusy}
            mode={mode}
            agentName={selectedAgent?.qualifiedName ?? selectedAgent?.name ?? 'Loopal'}
            runtimeStatus={props.detail ? (
              <SessionRuntimeStatus detail={props.detail} agentId={selectedAgent?.id ?? 'main'} />
            ) : undefined}
            commands={props.commands}
            {...(props.commandError ? { commandError: props.commandError } : {})}
            {...(props.commandHelpQuery !== undefined
              ? { commandHelpQuery: props.commandHelpQuery } : {})}
            onRequestCommands={props.onRequestCommands}
            onDismissCommandHelp={props.onDismissCommandHelp}
            onExecuteCommand={(command) => props.onExecuteCommand(
              command, selectedAgent?.id ?? props.selectedAgentId,
            )}
            onDraftChange={props.onDraftChange}
            onSelectImages={() => void props.onSelectImages()}
            onRemoveImage={props.onRemoveImage}
            onSend={() => void props.onSend(selectedAgent?.id ?? props.selectedAgentId)}
            onModeChange={(value) => control({ type: 'mode', mode: value })}
            onStopSession={() => void props.onStop()}
            onRestartSession={() => void props.onRestart()}
          />
        </>
      )}
    </main>
  )
}
