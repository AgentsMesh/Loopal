import { useEffect, useRef, useState } from 'react'
import {
  UpsertGlobalSkillInputSchema,
  type LoopalDesktopAPI,
  type PluginsResponse,
  type SkillsResponse,
} from '../../../../shared/contracts'
import { GlobalSkillEditor, type GlobalSkillDraft } from './global-skill-editor'
import { useI18n } from '../../../browser/i18n-context'
import { EffectiveSkillList, GlobalSkillList, PluginList } from './skill-plugin-lists'
import './skill-plugin-settings.css'

export function LoopalSkillPluginSettings(props: {
  readonly api: LoopalDesktopAPI
  readonly workspaceId?: string
  readonly visible?: boolean
}): React.JSX.Element | null {
  const { t } = useI18n()
  const [skills, setSkills] = useState<SkillsResponse>()
  const [plugins, setPlugins] = useState<PluginsResponse>()
  const [draft, setDraft] = useState<GlobalSkillDraft>()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const [message, setMessage] = useState<string>()
  const createButton = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    let active = true
    setSkills(undefined); setPlugins(undefined); setDraft(undefined)
    setError(undefined); setMessage(undefined)
    if (!props.workspaceId) return () => { active = false }
    void Promise.all([
      props.api.listSkills(props.workspaceId), props.api.listPlugins(props.workspaceId),
    ]).then(([nextSkills, nextPlugins]) => {
      if (!active) return
      setSkills(nextSkills); setPlugins(nextPlugins)
    }, (reason) => { if (active) setError(errorText(reason)) })
    return () => { active = false }
  }, [props.api, props.workspaceId])

  const edit = async (name: string): Promise<void> => {
    if (!props.workspaceId) return
    setBusy(true); setError(undefined); setMessage(undefined)
    try {
      const skill = await props.api.getSkill({ workspaceId: props.workspaceId, name })
      setDraft(skill)
    } catch (reason) {
      setError(errorText(reason))
    } finally {
      setBusy(false)
    }
  }
  const save = async (): Promise<void> => {
    if (!props.workspaceId || !draft) return
    setBusy(true); setError(undefined); setMessage(undefined)
    try {
      const input = UpsertGlobalSkillInputSchema.parse({
        workspaceId: props.workspaceId, name: draft.name,
        description: draft.description, body: draft.body,
        ...(draft.revision ? { expectedRevision: draft.revision } : {}),
      })
      const next = await props.api.upsertGlobalSkill(input)
      setDraft(next)
      setSkills(await props.api.listSkills(props.workspaceId))
      setMessage(t('settings.skills.saved', { name: next.name }))
    } catch (reason) {
      setError(errorText(reason))
    } finally {
      setBusy(false)
    }
  }
  const remove = async (): Promise<void> => {
    if (!props.workspaceId || !draft?.revision) return
    setBusy(true); setError(undefined); setMessage(undefined)
    try {
      const next = await props.api.deleteGlobalSkill({
        workspaceId: props.workspaceId, name: draft.name, expectedRevision: draft.revision,
      })
      setSkills(next); setDraft(undefined)
      setMessage(t('settings.skills.deleted', { name: draft.name }))
      queueMicrotask(() => createButton.current?.focus())
    } catch (reason) {
      setError(errorText(reason))
    } finally {
      setBusy(false)
    }
  }
  if (props.visible === false) return null
  return <section className="settings-section skills-plugin-settings"
    data-testid="skills-plugin-settings" aria-labelledby="skills-plugin-title"
    aria-busy={busy}>
    <div className="skills-plugin-heading">
      <div><h3 id="skills-plugin-title">{t('settings.skills.title')}</h3>
        <p className="muted">{t('settings.skills.help')}</p></div>
      {skills && !draft && <button ref={createButton} type="button" data-testid="skill-create"
        disabled={busy} onClick={() => {
          setDraft({ name: '/', description: '', body: '' })
          setError(undefined); setMessage(undefined)
        }}>{t('settings.skills.global.create')}</button>}
    </div>
    {!props.workspaceId && <p className="muted">{t('settings.skills.openWorkspace')}</p>}
    {props.workspaceId && (!skills || !plugins) && !error
      && <p className="muted" role="status">{t('settings.skills.loading')}</p>}
    {draft && props.workspaceId && <GlobalSkillEditor workspaceId={props.workspaceId}
      draft={draft} busy={busy} onChange={setDraft} onSave={() => void save()}
      onDelete={() => void remove()} onCancel={() => {
        setDraft(undefined); queueMicrotask(() => createButton.current?.focus())
      }} />}
    {skills && !draft && <>
      <GlobalSkillList skills={skills.skills} busy={busy} onEdit={(name) => void edit(name)} />
      <p className="skill-next-call-note">{t('settings.skills.nextCall')}</p>
      <EffectiveSkillList skills={skills.skills} />
    </>}
    {plugins && !draft && <PluginList plugins={plugins.plugins} />}
    {error && <p role="alert" className="diagnostic-error">{error}</p>}
    {message && <p role="status">{message}</p>}
  </section>
}

function errorText(value: unknown): string {
  return value instanceof Error ? value.message : String(value)
}
