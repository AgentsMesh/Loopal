import { expect, type Page } from '@playwright/test'
import { type SessionDetail } from '../../../src/shared/contracts'
import { waitForHostStatus } from './host-status'

export interface RuntimeTarget {
  readonly sessionId: string
  readonly runtimeId: string
  readonly generation: number
  readonly agentId: string
}

export async function ready(page: Page): Promise<void> {
  await waitForHostStatus(page, 'ready')
  await expect(page.getByTestId('runtime-status')).toContainText(
    'Ready for input', { timeout: 30_000 },
  )
  await expect(page.getByLabel('Message Loopal')).toBeEnabled({ timeout: 30_000 })
}

export async function send(page: Page, message: string): Promise<void> {
  await page.getByLabel('Message Loopal').fill(message)
  await page.getByRole('button', { name: 'Send' }).click()
  await expect(page.getByTestId('conversation')).toContainText(message)
}

export async function activeDetail(page: Page): Promise<SessionDetail> {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    return window.loopalDesktop.openSession(bootstrap.activeSessionId!)
  })
}

export async function runtimeTarget(page: Page): Promise<RuntimeTarget> {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    const sessionId = bootstrap.activeSessionId!
    const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)!
    return {
      sessionId, runtimeId: runtime.id, generation: runtime.generation,
      agentId: runtime.rootAgent,
    }
  })
}
