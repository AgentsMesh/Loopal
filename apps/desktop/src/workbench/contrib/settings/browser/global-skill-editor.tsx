import { useState } from 'react'
import { UpsertGlobalSkillInputSchema } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

export interface GlobalSkillDraft {
  readonly name: string
  readonly description: string
  readonly body: string
  readonly revision?: string
}

export function GlobalSkillEditor(props: {
  readonly workspaceId: string
  readonly draft: GlobalSkillDraft
  readonly busy: boolean
  readonly onChange: (draft: GlobalSkillDraft) => void
  readonly onSave: () => void
  readonly onDelete: () => void
  readonly onCancel: () => void
}): React.JSX.Element {
  const { t } = useI18n()
  const [confirmDelete, setConfirmDelete] = useState(false)
  const editing = props.draft.revision !== undefined
  const valid = UpsertGlobalSkillInputSchema.safeParse({
    workspaceId: props.workspaceId,
    name: props.draft.name,
    description: props.draft.description,
    body: props.draft.body,
    ...(props.draft.revision ? { expectedRevision: props.draft.revision } : {}),
  }).success
  const update = (patch: Partial<GlobalSkillDraft>): void => {
    props.onChange({ ...props.draft, ...patch })
  }
  return <section className="skill-editor" aria-labelledby="skill-editor-title">
    <h4 id="skill-editor-title">{t(editing
      ? 'settings.skills.editor.edit' : 'settings.skills.editor.create')}</h4>
    <label className="settings-field">
      <span>{t('settings.skills.editor.name')}</span>
      <input data-testid="skill-name" value={props.draft.name} disabled={editing || props.busy}
        autoFocus={!editing} aria-describedby="skill-name-hint" placeholder="/review"
        onChange={(event) => update({ name: event.target.value })} />
      <small id="skill-name-hint">{t('settings.skills.editor.nameHint')}</small>
    </label>
    <label className="settings-field">
      <span>{t('settings.skills.editor.description')}</span>
      <input data-testid="skill-description" value={props.draft.description}
        disabled={props.busy} maxLength={512} autoFocus={editing}
        onChange={(event) => update({ description: event.target.value })} />
    </label>
    <label className="settings-field">
      <span>{t('settings.skills.editor.body')}</span>
      <textarea data-testid="skill-body" value={props.draft.body} disabled={props.busy}
        required maxLength={100 * 1024} rows={10} aria-describedby="skill-body-hint"
        onChange={(event) => update({ body: event.target.value })} />
      <small id="skill-body-hint">{t('settings.skills.editor.bodyHint')}</small>
    </label>
    <div className="settings-actions">
      <button type="button" data-testid="skill-save" disabled={props.busy || !valid}
        onClick={props.onSave}>{t('settings.skills.editor.save')}</button>
      <button type="button" disabled={props.busy} onClick={props.onCancel}>
        {t('settings.skills.editor.cancel')}
      </button>
      {editing && <button type="button" className="danger" data-testid="skill-delete"
        disabled={props.busy} onClick={() => setConfirmDelete(true)}>
        {t('settings.skills.editor.delete')}
      </button>}
    </div>
    {confirmDelete && <div className="skill-delete-confirmation" role="alertdialog"
      aria-labelledby="skill-delete-title">
      <p id="skill-delete-title">{t('settings.skills.editor.confirmDelete', {
        name: props.draft.name,
      })}</p>
      <div className="settings-actions">
        <button type="button" className="danger" disabled={props.busy}
          onClick={props.onDelete}>{t('settings.skills.editor.confirm')}</button>
        <button type="button" disabled={props.busy} onClick={() => setConfirmDelete(false)}>
          {t('settings.skills.editor.cancel')}
        </button>
      </div>
    </div>}
  </section>
}
