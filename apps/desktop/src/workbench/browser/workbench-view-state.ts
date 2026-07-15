export type WorkbenchArea = 'conversation' | 'federation'

export interface WorkbenchViewState {
  readonly area: WorkbenchArea
  readonly sidebarVisible: boolean
  readonly settingsOpen: boolean
}

export type WorkbenchViewAction =
  | { readonly type: 'select_area'; readonly area: WorkbenchArea }
  | { readonly type: 'toggle_sidebar' }
  | { readonly type: 'toggle_settings' }
  | { readonly type: 'open_settings' }
  | { readonly type: 'close_settings' }

export function createWorkbenchViewState(): WorkbenchViewState {
  return {
    area: 'conversation',
    sidebarVisible: true,
    settingsOpen: false,
  }
}

export function reduceWorkbenchView(
  state: WorkbenchViewState,
  action: WorkbenchViewAction,
): WorkbenchViewState {
  switch (action.type) {
    case 'select_area':
      return { ...state, area: action.area, settingsOpen: false }
    case 'toggle_sidebar':
      return { ...state, sidebarVisible: !state.sidebarVisible }
    case 'toggle_settings':
      return { ...state, settingsOpen: !state.settingsOpen }
    case 'open_settings':
      return { ...state, settingsOpen: true }
    case 'close_settings':
      return { ...state, settingsOpen: false }
  }
}
