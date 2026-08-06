import {
  type LoopalSettingsValues,
  type LoopalThinking,
} from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

interface Props {
  readonly value: LoopalSettingsValues
  readonly disabled: boolean
  readonly onChange: (patch: Partial<LoopalSettingsValues>) => void
}

export function LoopalSettingsForm(props: Props): React.JSX.Element {
  const { t } = useI18n()
  const update = props.onChange
  const thinking = thinkingKey(props.value.thinking)
  return <div className="loopal-settings-form">
    <TextField label={t('settings.loopal.defaultModel')} value={props.value.model} maxLength={256} disabled={props.disabled}
      onChange={(model) => update({ model })} />
    <fieldset className="settings-fieldset">
      <legend>{t('settings.loopal.routing')}</legend>
      {([
        ['default', t('settings.loopal.routing.conversation')],
        ['summarization', t('settings.loopal.routing.summarization')],
        ['classification', t('settings.loopal.routing.classification')],
        ['refine', t('settings.loopal.routing.refine')],
      ] as const).map(([key, label]) => <TextField key={key} label={label}
        value={props.value.modelRouting[key]} maxLength={256} disabled={props.disabled}
        onChange={(value) => update({
          modelRouting: { ...props.value.modelRouting, [key]: value },
        })} />)}
    </fieldset>
    <SelectField label={t('settings.loopal.permission')} value={props.value.permissionMode}
      disabled={props.disabled} options={[
        ['bypass', t('settings.loopal.permission.bypass')],
        ['ask_dangerous', t('settings.loopal.permission.dangerous')],
        ['ask_any_write', t('settings.loopal.permission.write')],
      ]} onChange={(permissionMode) => update({ permissionMode })} />
    <SelectField label={t('settings.loopal.decision')} value={props.value.decisionMode}
      disabled={props.disabled} options={[
        ['manual', t('settings.loopal.decision.manual')],
        ['classifier', t('settings.loopal.decision.classifier')],
        ['agent', t('settings.loopal.decision.agent')],
      ]} onChange={(decisionMode) => update({ decisionMode })} />
    <SelectField label={t('settings.loopal.sandbox')} value={props.value.sandboxPolicy}
      disabled={props.disabled} options={[
        ['default_write', t('settings.loopal.sandbox.write')],
        ['read_only', t('settings.loopal.sandbox.readOnly')],
        ['disabled', t('settings.loopal.disabled')],
      ]} onChange={(sandboxPolicy) => update({ sandboxPolicy })} />
    <SelectField label={t('settings.loopal.thinking')} value={thinking} disabled={props.disabled}
      options={[
        ['auto', t('settings.loopal.auto')], ['disabled', t('settings.loopal.disabled')],
        ['effort:none', t('settings.loopal.thinking.none')],
        ['effort:low', t('settings.loopal.thinking.low')],
        ['effort:medium', t('settings.loopal.thinking.medium')],
        ['effort:high', t('settings.loopal.thinking.high')],
        ['effort:xhigh', t('settings.loopal.thinking.xhigh')],
        ['effort:max', t('settings.loopal.thinking.max')],
        ['budget', t('settings.loopal.thinking.budget')],
      ]} onChange={(next) => update({ thinking: parseThinking(next, props.value.thinking) })} />
    {props.value.thinking.type === 'budget' && <NumberField
      label={t('settings.loopal.thinking.tokens')} value={props.value.thinking.tokens} min={1}
      max={4_294_967_295} disabled={props.disabled}
      onChange={(tokens) => update({ thinking: { type: 'budget', tokens } })} />}
    <NumberField label={t('settings.loopal.contextTokens')}
      value={props.value.maxContextTokens} min={0} max={4_294_967_295}
      disabled={props.disabled} onChange={(maxContextTokens) => update({ maxContextTokens })} />
    <NumberField label={t('settings.loopal.microcompact')}
      value={props.value.microcompactIdleMinutes} min={0} max={1440}
      disabled={props.disabled}
      onChange={(microcompactIdleMinutes) => update({ microcompactIdleMinutes })} />
    <CheckField label={t('settings.loopal.memory')} checked={props.value.memoryEnabled}
      disabled={props.disabled} onChange={(memoryEnabled) => update({ memoryEnabled })} />
    <CheckField label={t('settings.loopal.telemetry')} checked={props.value.telemetryEnabled}
      disabled={props.disabled} onChange={(telemetryEnabled) => update({ telemetryEnabled })} />
    <TextField label={t('settings.loopal.outputStyle')} value={props.value.outputStyle} maxLength={128} disabled={props.disabled}
      onChange={(outputStyle) => update({ outputStyle })} />
  </div>
}

function TextField(props: {
  label: string; value: string; maxLength: number; disabled: boolean; onChange(value: string): void
}) {
  return <label className="settings-field"><span>{props.label}</span><input
    aria-label={props.label} value={props.value} maxLength={props.maxLength} disabled={props.disabled}
    onChange={(event) => props.onChange(event.currentTarget.value)} /></label>
}

function NumberField(props: {
  label: string; value: number; min: number; max: number; disabled: boolean
  onChange(value: number): void
}) {
  return <label className="settings-field"><span>{props.label}</span><input type="number"
    aria-label={props.label} value={props.value} min={props.min} max={props.max}
    disabled={props.disabled}
    onChange={(event) => props.onChange(Number(event.currentTarget.value))} /></label>
}

function SelectField<T extends string>(props: {
  label: string; value: T; options: readonly (readonly [T, string])[]; disabled: boolean
  onChange(value: T): void
}) {
  return <label className="settings-field"><span>{props.label}</span><select
    aria-label={props.label} value={props.value} disabled={props.disabled}
    onChange={(event) => props.onChange(event.currentTarget.value as T)}>
    {props.options.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
  </select></label>
}

function CheckField(props: {
  label: string; checked: boolean; disabled: boolean; onChange(value: boolean): void
}) {
  return <label className="settings-check"><input type="checkbox" aria-label={props.label}
    checked={props.checked} disabled={props.disabled}
    onChange={(event) => props.onChange(event.currentTarget.checked)} /><span>{props.label}</span></label>
}

function thinkingKey(value: LoopalThinking): string {
  return value.type === 'effort' ? `effort:${value.level}` : value.type
}

function parseThinking(value: string, current: LoopalThinking): LoopalThinking {
  if (value === 'auto' || value === 'disabled') return { type: value }
  if (value === 'budget') return current.type === 'budget' ? current : { type: 'budget', tokens: 4096 }
  return { type: 'effort', level: value.slice(7) as 'none' | 'low' | 'medium' | 'high' | 'xhigh' | 'max' }
}
