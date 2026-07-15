import { useEffect, useRef, useState } from 'react'
import {
  type CreateSessionInput, type LoopalDesktopAPI, type SessionDirectorySelection,
} from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

type LaunchMode = 'directory' | 'worktree'
const worktreePattern = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/u

export function SessionCreateDialog(props: {
  readonly api: Pick<LoopalDesktopAPI, 'selectSessionDirectory'>
  readonly onCreate: (input: CreateSessionInput) => Promise<string | undefined>
  readonly onClose: () => void
}): React.JSX.Element {
  const { t } = useI18n()
  const dialogRef = useRef<HTMLDivElement>(null)
  const closeRef = useRef(props.onClose)
  const creatingRef = useRef(false)
  const [selection, setSelection] = useState<SessionDirectorySelection>()
  const [launchMode, setLaunchMode] = useState<LaunchMode>('directory')
  const [worktreeName, setWorktreeName] = useState('')
  const [picking, setPicking] = useState(false)
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string>()
  const invalidName = launchMode === 'worktree' && !worktreePattern.test(worktreeName)
  closeRef.current = props.onClose
  creatingRef.current = creating

  useEffect(() => {
    const dialog = dialogRef.current
    const previous = document.activeElement as HTMLElement | null
    if (!dialog) return
    const keydown = (event: KeyboardEvent): void => {
      if (event.key === 'Escape' && !creatingRef.current) {
        event.preventDefault()
        closeRef.current()
      }
      if (event.key !== 'Tab') return
      const focusable = focusableElements(dialog)
      const index = focusable.indexOf(document.activeElement as HTMLElement)
      if (focusable.length && (index < 0
        || (!event.shiftKey && index === focusable.length - 1)
        || (event.shiftKey && index === 0))) {
        event.preventDefault()
        focusable[event.shiftKey ? focusable.length - 1 : 0]?.focus()
      }
    }
    dialog.addEventListener('keydown', keydown)
    requestAnimationFrame(() => dialog.querySelector<HTMLElement>('[data-testid="session-directory"]')
      ?.focus())
    return () => {
      dialog.removeEventListener('keydown', keydown)
      requestAnimationFrame(() => { if (previous?.isConnected) previous.focus() })
    }
  }, [])

  const chooseDirectory = async (): Promise<void> => {
    setPicking(true)
    setError(undefined)
    try {
      const selected = await props.api.selectSessionDirectory()
      if (!selected) return
      setSelection(selected)
      setLaunchMode('directory')
      setWorktreeName(selected.suggestedWorktreeName)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setPicking(false)
    }
  }
  const create = async (): Promise<void> => {
    if (!selection || invalidName || creating) return
    setCreating(true)
    setError(undefined)
    const input: CreateSessionInput = launchMode === 'worktree'
      ? { authorizationId: selection.authorizationId, launchMode, worktreeName }
      : { authorizationId: selection.authorizationId, launchMode }
    try {
      const createError = await props.onCreate(input)
      if (createError) setError(createError)
      else props.onClose()
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setCreating(false)
    }
  }

  return <div className="session-create-overlay">
    <div ref={dialogRef} className="session-create-dialog" role="dialog" aria-modal="true"
      aria-labelledby="session-create-title" data-testid="new-session-dialog">
      <header>
        <div><h2 id="session-create-title">{t('session.create.title')}</h2>
          <p>{t('session.create.subtitle')}</p></div>
        <button aria-label={t('common.close')} disabled={creating} onClick={props.onClose}>×</button>
      </header>
      <section>
        <div className="session-create-section-title"><strong>{t('session.create.directory')}</strong>
          <small>{t('session.create.directoryHint')}</small></div>
        <button className="session-directory" data-testid="session-directory"
          disabled={picking || creating} onClick={() => void chooseDirectory()}>
          <span>{selection?.path ?? t('session.create.chooseDirectory')}</span>
          {selection && <small>{t('session.create.changeDirectory')}</small>}
        </button>
        {selection && <div className="session-directory-meta">
          <strong>{selection.name}</strong>
          <span>{selection.git ? t('session.create.gitRepository') : t('session.create.plainDirectory')}</span>
          {selection.git?.branch && <span>{t('session.create.gitBranch', {
            branch: selection.git.branch,
          })}</span>}
        </div>}
      </section>
      {selection && <section>
        <div className="session-create-section-title"><strong>{t('session.create.launchMode')}</strong></div>
        <LaunchChoice id="launch-direct" checked={launchMode === 'directory'}
          title={t('session.create.direct')} hint={t('session.create.directHint')}
          disabled={creating} onSelect={() => setLaunchMode('directory')} />
        {selection.git && <LaunchChoice id="launch-worktree" checked={launchMode === 'worktree'}
          title={t('session.create.worktree')} hint={t('session.create.worktreeHint')}
          disabled={creating} onSelect={() => setLaunchMode('worktree')} />}
        {launchMode === 'worktree' && <label className="worktree-name">
          <span>{t('session.create.worktreeName')}</span>
          <input data-testid="worktree-name" value={worktreeName} disabled={creating}
            placeholder={t('session.create.worktreePlaceholder')}
            aria-invalid={invalidName} onChange={(event) => setWorktreeName(event.target.value)} />
          <small>{invalidName ? t('session.create.invalidWorktreeName')
            : t('session.create.worktreeNameHint')}</small>
        </label>}
        {launchMode === 'worktree'
          && <p className="session-create-warning">{t('session.create.dirtyWorktree')}</p>}
      </section>}
      {!selection && <p className="session-create-empty">{t('session.create.noDirectory')}</p>}
      {error && <p className="session-create-error" role="alert" data-testid="session-create-error">
        {t('session.create.error', { message: error })}</p>}
      <footer>
        <button data-testid="create-session-cancel" disabled={creating}
          onClick={props.onClose}>{t('common.cancel')}</button>
        <button className="primary" data-testid="create-session-confirm"
          disabled={!selection || invalidName || creating}
          onClick={() => void create()}>{creating ? t('session.create.creating')
            : t('session.create.confirm')}</button>
      </footer>
    </div>
  </div>
}

function LaunchChoice(props: {
  readonly id: string; readonly checked: boolean; readonly title: string; readonly hint: string
  readonly disabled: boolean; readonly onSelect: () => void
}): React.JSX.Element {
  return <label className={`launch-choice ${props.checked ? 'selected' : ''}`}>
    <input type="radio" name="launch-mode" data-testid={props.id} checked={props.checked}
      disabled={props.disabled} onChange={props.onSelect} />
    <span><strong>{props.title}</strong><small>{props.hint}</small></span>
  </label>
}

function focusableElements(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled)')]
}
function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}
