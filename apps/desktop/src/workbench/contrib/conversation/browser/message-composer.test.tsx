import { render, screen, within } from '@testing-library/react'
import { type ComponentProps } from 'react'
import { I18nProvider } from '../../../browser/i18n-context'
import { MessageComposer } from './message-composer'

const callback = (): void => undefined

function composerProps(
  overrides: Partial<ComponentProps<typeof MessageComposer>> = {},
): ComponentProps<typeof MessageComposer> {
  return {
    label: 'Message Loopal', placeholder: 'Ask Loopal', draft: '', images: [],
    disabled: false, sending: false, canControl: true, hasSession: true,
    sessionLive: true, canRestartSession: true, lifecycleBusy: false,
    mode: 'act', agentName: 'Loopal', onDraftChange: callback, onSend: callback,
    onSelectImages: callback, onRemoveImage: callback, onModeChange: callback,
    onStopSession: callback, onRestartSession: callback, ...overrides,
  }
}

describe('MessageComposer lifecycle controls', () => {
  it('keeps the compact runtime state inside the composer footer', () => {
    render(<MessageComposer {...composerProps({
      runtimeStatus: <span data-testid="compact-runtime">Thinking · Context 20%</span>,
    })} />)
    const composer = screen.getByTestId('message-composer')
    expect(within(composer).getByTestId('compact-runtime')).toBeVisible()
  })

  it('keeps lifecycle actions in the composer and gates every session state', () => {
    const onStopSession = vi.fn()
    const onRestartSession = vi.fn()
    const view = render(<MessageComposer {...composerProps({
      onStopSession, onRestartSession,
    })} />)
    const composer = within(screen.getByTestId('message-composer'))
    expect(composer.getByRole('button', { name: 'Stop session' })).toBeEnabled()
    expect(composer.getByRole('button', { name: 'Restart session' })).toBeEnabled()
    composer.getByRole('button', { name: 'Stop session' }).click()
    composer.getByRole('button', { name: 'Restart session' }).click()
    expect(onStopSession).toHaveBeenCalledOnce()
    expect(onRestartSession).toHaveBeenCalledOnce()

    view.rerender(<MessageComposer {...composerProps({
      sessionLive: false, canRestartSession: true,
    })} />)
    expect(composer.getByRole('button', { name: 'Stop session' })).toBeDisabled()
    expect(composer.getByRole('button', { name: 'Restart session' })).toBeEnabled()

    view.rerender(<MessageComposer {...composerProps({
      sessionLive: true, lifecycleBusy: true,
    })} />)
    expect(composer.getByRole('button', { name: 'Stop session' })).toBeDisabled()
    expect(composer.getByRole('button', { name: 'Restart session' })).toBeDisabled()

    view.rerender(<MessageComposer {...composerProps({
      sessionLive: false, canRestartSession: false,
    })} />)
    expect(composer.getByRole('button', { name: 'Stop session' })).toBeDisabled()
    expect(composer.getByRole('button', { name: 'Restart session' })).toBeDisabled()

    view.rerender(<MessageComposer {...composerProps({ hasSession: false })} />)
    expect(composer.queryByRole('button', { name: 'Stop session' })).not.toBeInTheDocument()
    expect(composer.queryByRole('button', { name: 'Restart session' })).not.toBeInTheDocument()
  })

  it('localizes lifecycle controls in Chinese', async () => {
    const api = {
      getDesktopPreferences: vi.fn(async () => ({ locale: 'zh-CN' as const })),
      updateDesktopPreferences: vi.fn(async () => ({ locale: 'zh-CN' as const })),
    }
    render(<I18nProvider api={api} systemLocales={['en-US']}>
      <MessageComposer {...composerProps()} />
    </I18nProvider>)
    expect(await screen.findByRole('button', { name: '停止会话' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '重启会话' })).toBeEnabled()
  })
})
