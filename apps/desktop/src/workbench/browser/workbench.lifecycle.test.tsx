import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { type DesktopEvent, type WorkbenchBootstrap } from '../../shared/contracts'
import {
  createTestAPI,
  sessionDetail,
  sessionOne,
  sessionTwo,
} from '../../../test/support/workbench/api-stub'
import { Workbench } from './workbench'

describe('Workbench lifecycle', () => {
  it('ignores bootstrap and host events after disposal', async () => {
    let resolveBootstrap: ((value: WorkbenchBootstrap) => void) | undefined
    let eventListener: ((event: DesktopEvent) => void) | undefined
    const unsubscribe = vi.fn()
    const openSession = vi.fn(async () => sessionDetail(sessionOne))
    const bootstrap = new Promise<WorkbenchBootstrap>((resolve) => {
      resolveBootstrap = resolve
    })
    const { api } = createTestAPI({
      bootstrap: () => bootstrap,
      openSession,
      onEvent: (listener) => {
        eventListener = listener
        return unsubscribe
      },
    })
    const view = render(<Workbench api={api} />)

    expect(screen.queryByTestId('session-panel-zone')).not.toBeInTheDocument()
    view.unmount()

    await act(async () => {
      eventListener?.({ type: 'host_status', status: 'ready' })
      resolveBootstrap?.({
        protocolVersion: 2,
        hostStatus: 'ready',
        workspaces: [],
        sessions: [sessionOne],
        runtimes: [],
        activeSessionId: sessionOne.id,
      })
      await bootstrap
    })
    expect(unsubscribe).toHaveBeenCalledTimes(3)
    expect(openSession).not.toHaveBeenCalled()
  })

  it('ignores late bootstrap failures and initial session results', async () => {
    let rejectBootstrap: ((reason: Error) => void) | undefined
    const bootstrap = new Promise<WorkbenchBootstrap>((_resolve, reject) => {
      rejectBootstrap = reject
    })
    const failed = createTestAPI({ bootstrap: () => bootstrap })
    const first = render(<Workbench api={failed.api} />)
    first.unmount()
    await act(async () => rejectBootstrap?.(new Error('late failure')))

    let resolveOpen: (() => void) | undefined
    const openSession = vi.fn(() => new Promise<ReturnType<typeof sessionDetail>>((resolve) => {
      resolveOpen = () => resolve(sessionDetail(sessionOne))
    }))
    const pending = createTestAPI({ openSession })
    const second = render(<Workbench api={pending.api} />)
    await waitFor(() => expect(openSession).toHaveBeenCalledOnce())
    second.unmount()
    await act(async () => resolveOpen?.())
  })

  it('ignores an open failure after the user selects another session', async () => {
    let rejectSessionTwo: ((reason: Error) => void) | undefined
    const openSession = vi.fn((sessionId: string) => {
      if (sessionId === sessionTwo.id) {
        return new Promise<ReturnType<typeof sessionDetail>>((_resolve, reject) => {
          rejectSessionTwo = reject
        })
      }
      return Promise.resolve(sessionDetail(sessionOne))
    })
    const { api } = createTestAPI({ openSession })
    render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)

    const sessionList = within(screen.getByTestId('session-list'))
    fireEvent.click(sessionList.getByText(sessionTwo.title))
    await waitFor(() => expect(openSession).toHaveBeenCalledWith(sessionTwo.id))
    fireEvent.click(sessionList.getByText(sessionOne.title))
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    await act(async () => rejectSessionTwo?.(new Error('stale failure')))

    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })

  it('replays events received while bootstrap is pending', async () => {
    let resolveBootstrap: ((value: WorkbenchBootstrap) => void) | undefined
    const pending = new Promise<WorkbenchBootstrap>((resolve) => { resolveBootstrap = resolve })
    const { api, events } = createTestAPI({ bootstrap: () => pending })
    render(<Workbench api={api} />)
    act(() => {
      events.fire({ type: 'host_status', status: 'alive' })
      events.fire({ type: 'session_updated', session: sessionTwo })
    })
    await act(async () => resolveBootstrap?.({
      protocolVersion: 2, hostStatus: 'ready', workspaces: [],
      sessions: [sessionOne], runtimes: [], activeSessionId: sessionOne.id,
    }))
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    expect(screen.queryByTestId('host-status')).not.toBeInTheDocument()
    expect(screen.getByTestId('session-list')).toHaveTextContent(sessionTwo.title)
  })
})
