import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'

test('opens a stopped real session read-only until explicit restart', async () => {
  const desktop = await launchDesktop('real', 'lifecycle')
  try {
    const page = desktop.page
    await waitForHostStatus(page, 'ready')
    const composer = page.getByLabel('Message Loopal')
    await expect(composer).toBeEnabled({ timeout: 30_000 })
    const initial = await page.evaluate(async () => {
      const bootstrap = await window.loopalDesktop.bootstrap()
      const sessionId = bootstrap.activeSessionId!
      const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)!
      return { sessionId, runtimeId: runtime.id, generation: runtime.generation }
    })

    await composer.fill('Retain this turn across Stop')
    await page.getByRole('button', { name: 'Send' }).click()
    await expect(page.getByTestId('conversation')).toContainText(
      'REAL LIFECYCLE RESPONSE', { timeout: 20_000 },
    )
    await page.getByRole('button', { name: 'Stop session', exact: true }).click()
    await waitForHostStatus(page, 'stopped', 20_000)
    await expect(page.getByRole('button', { name: 'Stop session', exact: true })).toBeDisabled()
    await expect(page.getByRole('button', { name: 'Restart session' })).toBeEnabled()
    await expect(composer).toBeDisabled()

    const stopped = await page.evaluate(async (sessionId) => {
      const first = await window.loopalDesktop.openSession(sessionId)
      const second = await window.loopalDesktop.openSession(sessionId)
      const bootstrap = await window.loopalDesktop.bootstrap()
      let sendError = ''
      try { await window.loopalDesktop.sendMessage(sessionId, 'must not spawn') }
      catch (error) { sendError = String(error) }
      return {
        first, second, sendError, hostStatus: bootstrap.hostStatus,
        runtimes: bootstrap.runtimes.filter((item) => item.sessionId === sessionId),
      }
    }, initial.sessionId)
    expect(stopped.first.session).toMatchObject({ status: 'stopped' })
    expect(stopped.first.session.activeRuntimeId).toBeUndefined()
    expect(stopped.first.conversation).toEqual(stopped.second.conversation)
    expect(stopped.first.conversation.map((entry) => entry.text).join('\n'))
      .toContain('REAL LIFECYCLE RESPONSE')
    expect(stopped.hostStatus).toBe('stopped')
    expect(stopped.runtimes).toEqual([
      expect.objectContaining({
        id: initial.runtimeId, generation: initial.generation, state: 'stopped',
      }),
    ])
    expect(stopped.sendError).toContain('restart it first')

    await page.getByRole('button', { name: 'Restart session', exact: true }).click()
    await waitForHostStatus(page, 'ready')
    await expect(page.getByRole('button', { name: 'Stop session', exact: true })).toBeEnabled()
    await expect(composer).toBeEnabled({ timeout: 30_000 })
    await expect.poll(async () => page.evaluate(async (sessionId) => {
      const detail = await window.loopalDesktop.openSession(sessionId)
      const bootstrap = await window.loopalDesktop.bootstrap()
      const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)
      return {
        runtimeId: runtime?.id, generation: runtime?.generation,
        state: runtime?.state,
        activeMatches: runtime?.id === detail.session.activeRuntimeId,
      }
    }, initial.sessionId)).toMatchObject({
      generation: initial.generation + 1, state: 'ready', activeMatches: true,
    })
    await composer.fill('Continue after the restarted runtime')
    await page.getByRole('button', { name: 'Send' }).click()
    await expect(page.getByTestId('conversation')).toContainText(
      'RESTARTED RUNTIME RETAINED MODEL CONTEXT', { timeout: 20_000 },
    )
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests[1]).toMatchObject({ messageCount: 3 })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
