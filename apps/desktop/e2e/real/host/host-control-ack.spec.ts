import { expect, test, type Page } from '@playwright/test'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'

test('acknowledges only controls applied by the real runtime', async () => {
  const desktop = await launchDesktop('real')
  try {
    const page = desktop.page
    await waitForHostStatus(page, 'ready')
    const target = await runtimeTarget(page)

    await page.evaluate(async (value) => {
      await window.loopalDesktop.controlAgent({
        target: value, command: { type: 'mode', mode: 'plan' },
      })
      await window.loopalDesktop.controlAgent({
        target: value, command: { type: 'permission', mode: 'bypass' },
      })
    }, target)
    await expect.poll(() => agentConfig(page, target)).toBe('plan/bypass')

    const unsupported = await controlError(page, target, {
      type: 'decision', mode: 'agent',
    })
    expect(unsupported).toContain("decision mode 'agent' is not implemented")
    await expect.poll(() => agentConfig(page, target)).toBe('plan/bypass')

    const stale = await controlError(page, { ...target, generation: target.generation + 1 }, {
      type: 'mode', mode: 'act',
    })
    expect(stale).toContain('Session runtime is gone')
    await expect.poll(() => agentConfig(page, target)).toBe('plan/bypass')
  } finally {
    await closeDesktop(desktop)
  }
})

interface Target {
  readonly sessionId: string
  readonly runtimeId: string
  readonly generation: number
  readonly agentId: string
}

async function runtimeTarget(page: Page): Promise<Target> {
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

async function agentConfig(page: Page, target: Target): Promise<string> {
  return page.evaluate(async (value) => {
    const detail = await window.loopalDesktop.openSession(value.sessionId)
    const agent = detail.agents.find((item) => item.id === value.agentId)
    return `${agent?.mode}/${agent?.permissionMode}`
  }, target)
}

async function controlError(
  page: Page,
  target: Target,
  command: { type: 'decision'; mode: 'agent' } | { type: 'mode'; mode: 'act' },
): Promise<string> {
  return page.evaluate(async ({ value, next }) => {
    try {
      await window.loopalDesktop.controlAgent({ target: value, command: next })
      return ''
    } catch (error) {
      return error instanceof Error ? error.message : String(error)
    }
  }, { value: target, next: command })
}
