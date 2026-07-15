import { describe, expect, it } from 'vitest'
import { IDesktopBackend } from './backend'

describe('IDesktopBackend', () => {
  it('publishes the stable runtime service identifier', () => {
    expect(IDesktopBackend.id).toBe('desktopBackend')
    expect(IDesktopBackend.toString()).toBe('desktopBackend')
  })
})
