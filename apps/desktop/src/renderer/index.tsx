import React from 'react'
import ReactDOM from 'react-dom/client'
import { Workbench } from '../workbench/browser/workbench'
import { I18nProvider } from '../workbench/browser/i18n-context'
import { detectRendererPlatform } from './renderer-platform'
import '../workbench/browser/workbench.css'

document.documentElement.dataset.platform = detectRendererPlatform(navigator.platform)
const root = document.getElementById('root')
if (!root) {
  throw new Error('Missing #root element')
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <I18nProvider>
      <Workbench api={window.loopalDesktop} />
    </I18nProvider>
  </React.StrictMode>,
)
