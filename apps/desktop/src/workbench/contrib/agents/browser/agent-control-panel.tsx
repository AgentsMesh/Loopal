import { useEffect, useState } from 'react'
import {
  type AgentControlCommand,
  type AgentSummary,
} from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

interface AgentControlPanelProps {
  readonly agent: AgentSummary
  readonly disabled: boolean
  readonly busy: boolean
  readonly onInterrupt: () => void
  readonly onControl: (command: AgentControlCommand) => void
}

const THINKING = ['auto', 'disabled', 'low', 'medium', 'high', 'max'] as const
const MODE = ['act', 'plan'] as const
const PERMISSION = ['ask_dangerous', 'ask_any_write', 'bypass'] as const
const DECISION = ['manual', 'classifier'] as const
const SANDBOX = ['default_write', 'read_only', 'disabled'] as const

export function AgentControlPanel(props: AgentControlPanelProps): React.JSX.Element {
  const { t } = useI18n()
  const [model, setModel] = useState(props.agent.model ?? '')
  const [compactInstructions, setCompactInstructions] = useState('')
  const [turnIndex, setTurnIndex] = useState(defaultTurn(props.agent.telemetry?.turnCount))
  const disabled = props.disabled || props.busy
  useEffect(() => {
    setModel(props.agent.model ?? '')
    setCompactInstructions('')
    setTurnIndex(defaultTurn(props.agent.telemetry?.turnCount))
  }, [props.agent.id, props.agent.model, props.agent.telemetry?.turnCount])
  const control = (command: AgentControlCommand): void => props.onControl(command)
  return (
    <div className="agent-control-panel" role="group" aria-label={t('agent.controls')}>
      <div className="control-panel-heading">
        <strong>{props.agent.name}</strong>
        <span>{props.busy ? t('agent.applying') : props.agent.status}</span>
      </div>
      <div className="control-quick-actions">
        <button disabled={disabled} aria-label={t('agent.interrupt')} onClick={props.onInterrupt}>
          {t('agent.interrupt')}
        </button>
        <button disabled={disabled} aria-label={props.agent.status === 'suspended' ? t('agent.unsuspend') : t('agent.suspend')} onClick={() => control({
          type: props.agent.status === 'suspended' ? 'unsuspend' : 'suspend',
        })}>{props.agent.status === 'suspended' ? t('agent.unsuspend') : t('agent.suspend')}</button>
        <button disabled={disabled} aria-label={t('agent.clear')} onClick={() => control({ type: 'clear' })}>
          {t('agent.clear')}
        </button>
      </div>
      <ControlSelect
        label={t('agent.mode')} ariaLabel={t('agent.agentMode')}
        value={props.agent.mode ?? ''} disabled={disabled}
        options={MODE}
        onChange={(mode) => control({ type: 'mode', mode })}
      />
      <ControlRow label={t('agent.model')}>
        <input
          aria-label={t('agent.agentModel')}
          value={model}
          maxLength={4_096}
          disabled={disabled}
          onChange={(event) => setModel(event.target.value)}
        />
        <button
          disabled={disabled || !model.trim()}
          aria-label={t('agent.applyModel')}
          onClick={() => control({ type: 'model', model: model.trim() })}
        >{t('agent.apply')}</button>
      </ControlRow>
      <ControlSelect
        label={t('agent.thinking')} ariaLabel={t('agent.thinkingConfig')}
        value={props.agent.thinkingConfig ?? ''} disabled={disabled}
        options={THINKING}
        onChange={(value) => control(value === 'auto' || value === 'disabled'
          ? { type: 'thinking', config: { type: value } }
          : { type: 'thinking', config: { type: 'effort', level: value } })}
      />
      <ControlSelect
        label={t('agent.permission')} ariaLabel={t('agent.permissionMode')}
        value={props.agent.permissionMode ?? ''} disabled={disabled}
        options={PERMISSION}
        onChange={(mode) => control({ type: 'permission', mode })}
      />
      <ControlSelect
        label={t('agent.decision')} ariaLabel={t('agent.decisionMode')}
        value={props.agent.decisionMode ?? ''} disabled={disabled}
        options={DECISION}
        onChange={(mode) => control({ type: 'decision', mode })}
      />
      <ControlSelect
        label={t('agent.sandbox')} ariaLabel={t('agent.sandboxPolicy')}
        value={props.agent.sandboxPolicy ?? ''} disabled={disabled}
        options={SANDBOX}
        onChange={(policy) => control({ type: 'sandbox', policy })}
      />
      <ControlRow label={t('agent.compact')}>
        <input
          aria-label={t('agent.compactInstructions')}
          placeholder={t('agent.optionalInstructions')}
          value={compactInstructions}
          maxLength={4_096}
          disabled={disabled}
          onChange={(event) => setCompactInstructions(event.target.value)}
        />
        <button disabled={disabled} aria-label={t('agent.compact')} onClick={() => control({
          type: 'compact',
          ...(compactInstructions.trim() ? { instructions: compactInstructions.trim() } : {}),
        })}>{t('agent.compact')}</button>
      </ControlRow>
      <ControlRow label={t('agent.rewind')}>
        <input
          aria-label={t('agent.rewindTurn')}
          type="number"
          min="0"
          max={Math.max(0, (props.agent.telemetry?.turnCount ?? 1) - 1)}
          step="1"
          value={turnIndex}
          disabled={disabled}
          onChange={(event) => setTurnIndex(event.target.value)}
        />
        <button
          disabled={disabled || !validTurn(turnIndex, props.agent.telemetry?.turnCount)}
          aria-label={t('agent.rewind')}
          onClick={() => control({ type: 'rewind', turnIndex: Number(turnIndex) })}
        >{t('agent.rewind')}</button>
      </ControlRow>
    </div>
  )
}

function ControlRow(props: {
  readonly label: string
  readonly children: React.ReactNode
}): React.JSX.Element {
  return <label className="agent-control-row"><span>{props.label}</span>{props.children}</label>
}

function ControlSelect<const T extends string>(props: {
  readonly label: string
  readonly ariaLabel: string
  readonly value: string
  readonly options: readonly T[]
  readonly disabled: boolean
  readonly onChange: (value: T) => void
}): React.JSX.Element {
  const { t } = useI18n()
  const observed = !props.options.some((option) => option === props.value)
  return (
    <label className="agent-control-row">
      <span>{props.label}</span>
      <select
        aria-label={props.ariaLabel} value={props.value} disabled={props.disabled}
        onChange={(event) => props.onChange(event.target.value as T)}
      >
        {observed && (
          <option value={props.value} disabled>
            {props.value === 'agent'
              ? t('agent.unsupportedMode')
              : props.value ? t('agent.observed', { value: props.value }) : t('agent.unavailable')}
          </option>
        )}
        {props.options.map((option) => <option key={option} value={option}>{label(option)}</option>)}
      </select>
    </label>
  )
}

function defaultTurn(turnCount: number | undefined): string {
  return String(Math.max(0, (turnCount ?? 1) - 1))
}
function validTurn(value: string, turnCount: number | undefined): boolean {
  const turn = Number(value)
  return Number.isSafeInteger(turn) && turn >= 0
    && (turnCount === undefined || turn < turnCount)
}
function label(value: string): string {
  return value.replaceAll('_', ' ').replace(/^./, (first) => first.toUpperCase())
}
