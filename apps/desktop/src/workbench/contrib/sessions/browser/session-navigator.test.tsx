import { createRef, type ComponentProps } from 'react'
import { fireEvent, render, screen } from '@testing-library/react'
import { type SessionSummary } from '../../../../shared/contracts'
import {
  SessionNavigator,
} from './session-navigator'
import { I18nProvider } from '../../../browser/i18n-context'
import { type FederationMembership } from './session-context-menu'

const timestamp = '2026-07-13T12:00:00.000Z'
const session: SessionSummary = {
  id: 'session-1', workspaceId: 'workspace-1', title: 'Loopal session',
  model: 'mock', mode: 'act', status: 'running',
  activeRuntimeId: 'runtime-1', createdAt: timestamp, updatedAt: timestamp,
}
const history: SessionSummary = {
  ...session, id: 'session-history', title: 'Archived session',
  status: 'archived', activeRuntimeId: undefined,
}

describe('SessionNavigator federation menu', () => {
  it('joins from a pointer menu without opening the session and restores focus', () => {
    const actions = renderNavigator('disconnected')
    const card = sessionCard()

    fireEvent.contextMenu(card, { clientX: 40, clientY: 60 })

    const item = screen.getByRole('menuitem', { name: 'Join Federation' })
    expect(screen.getByTestId('session-context-menu')).toBeInTheDocument()
    expect(item).toHaveFocus()
    expect(actions.open).not.toHaveBeenCalled()
    fireEvent.click(item)
    expect(actions.join).toHaveBeenCalledWith('session-1')
    expect(actions.leave).not.toHaveBeenCalled()
    expect(screen.queryByTestId('session-context-menu')).not.toBeInTheDocument()
    expect(card).toHaveFocus()
  })

  it('leaves a connected federation from the session menu', () => {
    const actions = renderNavigator('connected')
    fireEvent.contextMenu(sessionCard())
    fireEvent.click(screen.getByRole('menuitem', { name: 'Leave Federation' }))
    expect(actions.leave).toHaveBeenCalledWith('session-1')
    expect(actions.join).not.toHaveBeenCalled()
    expect(actions.open).not.toHaveBeenCalled()
  })

  it('supports keyboard opening, arrows, escape, and outside dismissal', () => {
    renderNavigator('disconnected')
    const card = sessionCard()
    card.focus()
    fireEvent.keyDown(card, { key: 'F10', shiftKey: true })
    const item = screen.getByRole('menuitem', { name: 'Join Federation' })
    fireEvent.keyDown(item, { key: 'ArrowDown' })
    expect(item).toHaveFocus()
    fireEvent.keyDown(item, { key: 'Escape' })
    expect(screen.queryByTestId('session-context-menu')).not.toBeInTheDocument()
    expect(card).toHaveFocus()

    fireEvent.keyDown(card, { key: 'ContextMenu' })
    expect(screen.getByTestId('session-context-menu')).toBeInTheDocument()
    fireEvent.pointerDown(document.body)
    expect(screen.queryByTestId('session-context-menu')).not.toBeInTheDocument()
    expect(card).toHaveFocus()
  })

  it('disables unavailable and busy federation actions', () => {
    const unavailable = renderNavigator('unavailable')
    fireEvent.contextMenu(sessionCard())
    const unavailableItem = screen.getByRole('menuitem', { name: 'Federation unavailable' })
    expect(unavailableItem).toBeDisabled()
    fireEvent.click(unavailableItem)
    expect(unavailable.join).not.toHaveBeenCalled()
    unavailable.view.unmount()

    const busy = renderNavigator('disconnected', 'session:session-1')
    fireEvent.contextMenu(sessionCard())
    const busyItem = screen.getByRole('menuitem', { name: 'Joining Federation…' })
    expect(busyItem).toBeDisabled()
    fireEvent.click(busyItem)
    expect(busy.join).not.toHaveBeenCalled()
  })
})

describe('SessionNavigator information architecture', () => {
  it('keeps history out of the default current-session list', () => {
    const onRequestCreate = vi.fn()
    renderCatalog({ currentSessions: [session], searchResults: [history], onRequestCreate })
    expect(screen.getByTestId('current-session-list')).toHaveTextContent('Loopal session')
    expect(screen.queryByTestId('history-session-list')).not.toBeInTheDocument()
    expect(screen.queryByText('Archived session')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'New Session' }))
    expect(onRequestCreate).toHaveBeenCalledOnce()
  })

  it('splits matching current and historical sessions only while searching', () => {
    renderCatalog({
      query: 'session', currentSessions: [session], searchResults: [session, history],
    })
    expect(screen.getByTestId('current-session-list')).toHaveTextContent('Loopal session')
    expect(screen.getByTestId('history-session-list')).toHaveTextContent('Archived session')
    expect(screen.getByRole('heading', { name: /Current sessions/ })).toBeVisible()
    expect(screen.getByRole('heading', { name: /History/ })).toBeVisible()
  })

  it('renders distinct empty states without empty section shells', () => {
    const view = renderCatalog({ currentSessions: [], searchResults: [] })
    expect(screen.getByText('No active sessions. Create one to start working.')).toBeVisible()
    expect(screen.queryByTestId('current-session-list')).not.toBeInTheDocument()
    view.rerender(navigator({ query: 'missing', currentSessions: [], searchResults: [] }))
    expect(screen.getByText('No sessions match “missing”.')).toBeVisible()
    expect(screen.queryByTestId('history-session-list')).not.toBeInTheDocument()
  })

  it('localizes section and empty-state language in Chinese', async () => {
    const api = {
      getDesktopPreferences: vi.fn(async () => ({ locale: 'zh-CN' as const })),
      updateDesktopPreferences: vi.fn(async () => ({ locale: 'zh-CN' as const })),
    }
    render(<I18nProvider api={api} systemLocales={['en-US']}>
      {navigator({ currentSessions: [session], searchResults: [] })}
    </I18nProvider>)
    expect(await screen.findByText('当前会话')).toBeVisible()
    expect(screen.getByLabelText('搜索会话'))
      .toHaveAttribute('placeholder', '搜索全部会话  ⌘K')
  })
})

function renderNavigator(membership: FederationMembership, busy?: string) {
  const open = vi.fn(async () => undefined)
  const join = vi.fn(async () => undefined)
  const leave = vi.fn(async () => undefined)
  const view = render(<SessionNavigator currentSessions={[session]} searchResults={[]}
    activeSessionId="session-1"
    query="" searchRef={createRef<HTMLInputElement>()} canCreate
    federation={{ memberships: { 'session-1': membership }, busy, onJoin: join, onLeave: leave }}
    onQueryChange={vi.fn()} onOpenSession={open} onRequestCreate={vi.fn()} />)
  return { view, open, join, leave }
}

function renderCatalog(overrides: Partial<ComponentProps<typeof SessionNavigator>>) {
  const view = render(navigator(overrides))
  return view
}

function navigator(
  overrides: Partial<ComponentProps<typeof SessionNavigator>> = {},
): React.JSX.Element {
  return <SessionNavigator currentSessions={[]} searchResults={[]} query=""
    searchRef={createRef<HTMLInputElement>()} canCreate federation={{
      memberships: {}, busy: undefined,
      onJoin: vi.fn(async () => undefined), onLeave: vi.fn(async () => undefined),
    }}
    onQueryChange={vi.fn()} onOpenSession={vi.fn(async () => undefined)}
    onRequestCreate={vi.fn()} {...overrides} />
}

function sessionCard(): HTMLButtonElement {
  const card = document.querySelector<HTMLButtonElement>('[data-session-id="session-1"]')
  if (!card) throw new Error('Missing session card')
  return card
}
