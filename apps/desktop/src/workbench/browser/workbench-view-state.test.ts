import {
  createWorkbenchViewState, reduceWorkbenchView,
} from './workbench-view-state'

describe('workbench view state', () => {
  it('starts in the conversation area', () => {
    expect(createWorkbenchViewState()).toEqual({
      area: 'conversation', sidebarVisible: true,
      settingsOpen: false,
    })
  })

  it('keeps utilities orthogonal while area changes close settings', () => {
    let state = createWorkbenchViewState()
    state = reduceWorkbenchView(state, { type: 'toggle_sidebar' })
    state = reduceWorkbenchView(state, { type: 'open_settings' })
    state = reduceWorkbenchView(state, { type: 'select_area', area: 'federation' })
    expect(state).toEqual({
      area: 'federation', sidebarVisible: false,
      settingsOpen: false,
    })
  })

  it('toggles settings without losing the underlying area', () => {
    const federation = reduceWorkbenchView(
      createWorkbenchViewState(),
      { type: 'select_area', area: 'federation' },
    )
    const open = reduceWorkbenchView(federation, { type: 'toggle_settings' })
    expect(open).toMatchObject({ area: 'federation', settingsOpen: true })
    expect(reduceWorkbenchView(open, { type: 'close_settings' }))
      .toMatchObject({ area: 'federation', settingsOpen: false })
  })
})
