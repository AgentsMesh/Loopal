import { type DesktopImageAttachment } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'
import { SlashCommandMenu } from './slash-command-menu'
import { type SlashCommandItem } from './slash-command-model'
import { useComposerCommandMenu } from './use-composer-command-menu'

interface MessageComposerProps {
  readonly label: string
  readonly placeholder: string
  readonly draft: string
  readonly images: readonly DesktopImageAttachment[]
  readonly disabled: boolean
  readonly sending: boolean
  readonly canControl: boolean
  readonly hasSession: boolean
  readonly sessionLive: boolean
  readonly canRestartSession: boolean
  readonly lifecycleBusy: boolean
  readonly mode: string | undefined
  readonly agentName: string
  readonly runtimeStatus?: React.ReactNode
  readonly commands?: readonly SlashCommandItem[]
  readonly commandError?: string
  readonly commandHelpQuery?: string
  readonly onRequestCommands?: () => void
  readonly onDismissCommandHelp?: () => void
  readonly onDraftChange: (value: string) => void
  readonly onSend: () => void
  readonly onExecuteCommand?: (command: string) => void
  readonly onSelectImages: () => void
  readonly onRemoveImage: (index: number) => void
  readonly onModeChange: (mode: 'act' | 'plan') => void
  readonly onStopSession: () => void
  readonly onRestartSession: () => void
}

export function MessageComposer(props: MessageComposerProps): React.JSX.Element {
  const { t } = useI18n()
  const supportedMode = props.mode === 'act' || props.mode === 'plan'
  const hasContent = Boolean(props.draft.trim() || props.images.length)
  const commands = useComposerCommandMenu({
    draft: props.draft,
    items: props.commands ?? [],
    helpQuery: props.commandHelpQuery,
    onDraftChange: props.onDraftChange,
    onExecuteCommand: props.onExecuteCommand,
    onRequestCommands: props.onRequestCommands,
    onDismissHelp: props.onDismissCommandHelp,
  })
  return (
    <footer className="composer" data-testid="message-composer">
      {commands.visible && (
        <SlashCommandMenu id={commands.menuId} label={t('slash.label')} items={commands.items}
          activeIndex={commands.activeIndex} emptyLabel={t('slash.empty')}
          onSelect={commands.select} onHover={commands.setActiveIndex} />
      )}
      {props.commandError && (
        <div className="command-error" role="alert" data-testid="command-error">
          {props.commandError}
        </div>
      )}
      {props.images.length > 0 && (
        <div className="pending-images" data-testid="pending-image-attachments">
          {props.images.map((image, index) => (
            <span className="pending-image" key={`${image.name}-${index}`}>
              <span title={image.name}>{image.name}</span>
              <button
                type="button"
                aria-label={t('composer.removeImage', { name: image.name })}
                disabled={props.disabled}
                onClick={() => props.onRemoveImage(index)}
              >×</button>
            </span>
          ))}
        </div>
      )}
      <textarea
        aria-label={props.label}
        placeholder={props.placeholder}
        value={props.draft}
        disabled={props.disabled}
        role="combobox"
        aria-autocomplete="list"
        aria-expanded={commands.visible}
        aria-controls={commands.visible ? commands.menuId : undefined}
        aria-activedescendant={commands.activeDescendant}
        onChange={(event) => props.onDraftChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) return
          if (commands.onKeyDown(event)) return
          if (event.key === 'Enter' && !event.shiftKey && hasContent) {
            event.preventDefault()
            props.onSend()
          }
        }}
      />
      <div className="composer-row">
        <div className="composer-tools">
          <button
            type="button"
            className="attach-images"
            aria-label={t('composer.attachImages')}
            disabled={props.disabled}
            onClick={props.onSelectImages}
          >{t('composer.image')}</button>
          {supportedMode ? (
            <label className="agent-mode-picker">
              <span>{t('composer.mode')}</span>
              <select
                aria-label={t('composer.agentMode')}
                value={props.mode}
                disabled={!props.canControl}
                onChange={(event) => props.onModeChange(event.target.value as 'act' | 'plan')}
              >
                <option value="act">{t('composer.act')}</option>
                <option value="plan">{t('composer.plan')}</option>
              </select>
              <span>· {props.agentName}</span>
            </label>
          ) : <span>{props.mode ?? t('composer.modeUnavailable')} · {props.agentName}</span>}
          {props.runtimeStatus}
        </div>
        <div className="composer-actions" aria-busy={props.lifecycleBusy}>
          {props.hasSession && <>
            <button type="button" className="composer-lifecycle"
              aria-label={t('composer.stopSession')}
              disabled={!props.sessionLive || props.lifecycleBusy}
              onClick={props.onStopSession}>{t('workspace.stop')}</button>
            <button type="button" className="composer-lifecycle"
              aria-label={t('composer.restartSession')}
              disabled={!props.canRestartSession || props.lifecycleBusy}
              onClick={props.onRestartSession}>{t('workspace.restart')}</button>
          </>}
          <button
            className="send-button"
            disabled={!hasContent || props.disabled}
            onClick={props.onSend}
          >{props.sending ? t('composer.running') : t('composer.send')}</button>
        </div>
      </div>
    </footer>
  )
}
