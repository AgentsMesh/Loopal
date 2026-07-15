import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { FakeDesktopBackend, type FakeBackendClock } from './fake-backend'

const clock: FakeBackendClock = {
  now: () => new Date('2026-07-12T00:00:00.000Z'),
  delay: async () => undefined,
}

describe('FakeDesktopBackend preferences', () => {
  let root: string | undefined

  afterEach(async () => {
    if (root) await rm(root, { recursive: true, force: true })
  })

  it('persists Desktop preferences when configured with a user data path', async () => {
    root = await mkdtemp(join(tmpdir(), 'loopal-desktop-fake-preferences-'))
    const path = join(root, 'desktop-preferences.json')
    const first = new FakeDesktopBackend(clock, path)
    expect(await first.getDesktopPreferences()).toEqual({ locale: 'system' })
    await first.updateDesktopPreferences({ locale: 'zh-CN' })
    first.dispose()

    const restored = new FakeDesktopBackend(clock, path)
    expect(await restored.getDesktopPreferences()).toEqual({ locale: 'zh-CN' })
    restored.dispose()
  })
})
