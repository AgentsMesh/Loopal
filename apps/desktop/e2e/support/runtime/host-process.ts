import { expect, type Page } from '@playwright/test'
import { readFile, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { shutdownDesktop, type DesktopFixture } from '../electron/electron-fixture'

export interface HostRecord {
  readonly pid: number
  readonly root_session_id: string
}

export async function sendMarker(page: Page, marker: string): Promise<void> {
  const composer = page.getByLabel('Message Loopal')
  await composer.fill(marker)
  await page.getByRole('button', { name: 'Send' }).click()
  await expect(page.getByTestId('conversation')).toContainText(marker)
  await expect(page.getByTestId('conversation')).toContainText('ok', { timeout: 30_000 })
}

export async function activeSessionId(page: Page): Promise<string> {
  const value = await page.locator('.session-card.selected').getAttribute('data-session-id')
  if (!value) throw new Error('No active Loopal Desktop Session')
  return value
}

export function sessionCard(page: Page, sessionId: string) {
  return page.locator(`[data-session-id="${sessionId}"]`)
}

export async function waitForHosts(home: string, count: number): Promise<readonly HostRecord[]> {
  let hosts: readonly HostRecord[] = []
  await expect.poll(async () => {
    hosts = await listHosts(home)
    return hosts.length
  }, { timeout: 30_000 }).toBe(count)
  return hosts
}

export async function waitForSessionHost(
  home: string,
  sessionId: string,
  excludedPid: number,
): Promise<HostRecord> {
  let result: HostRecord | undefined
  await expect.poll(async () => {
    result = (await listHosts(home)).find((item) => (
      item.root_session_id === sessionId && item.pid !== excludedPid
    ))
    return result?.pid ?? 0
  }, { timeout: 30_000 }).toBeGreaterThan(0)
  return result!
}

export async function listHosts(home: string): Promise<readonly HostRecord[]> {
  const directory = join(home, '.loopal', 'run')
  const files = await readdir(directory).catch(() => [])
  const records = await Promise.all(files.filter((file) => /^\d+\.json$/.test(file)).map(
    async (file) => {
      try {
        return JSON.parse(await readFile(join(directory, file), 'utf8')) as HostRecord
      } catch {
        return undefined
      }
    },
  ))
  return records.filter((item): item is HostRecord => (
    item !== undefined && Number.isSafeInteger(item.pid) && Boolean(item.root_session_id)
  ))
}

export async function shutdownAndAssertClean(
  desktop: DesktopFixture,
  pids: ReadonlySet<number>,
): Promise<void> {
  await shutdownDesktop(desktop)
  await expect.poll(() => [...pids].some(processIsRunning), { timeout: 10_000 }).toBe(false)
  await expect.poll(() => listHosts(desktop.home).then((items) => items.length)).toBe(0)
}

export function processIsRunning(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

export async function storedSession(
  home: string,
  id: string,
): Promise<{ id: string; cwd: string }> {
  return JSON.parse(await readFile(join(home, '.loopal', 'sessions', id, 'session.json'), 'utf8'))
}
