import { expect, type Page } from '@playwright/test'

export async function waitForHostStatus(
  page: Page, status: string, timeout = 30_000,
): Promise<void> {
  await expect.poll(() => page.evaluate(async () => (
    await window.loopalDesktop.bootstrap()
  ).hostStatus), { timeout }).toBe(status)
}
