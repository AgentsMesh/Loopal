import { type LoopalDesktopAPI } from './contracts'

declare global {
  interface Window {
    readonly loopalDesktop: LoopalDesktopAPI
  }
}

export {}
