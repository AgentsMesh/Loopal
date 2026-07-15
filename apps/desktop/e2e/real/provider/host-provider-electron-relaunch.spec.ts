import { expect, test, type Page } from '@playwright/test'
import {
  closeDesktop, launchDesktop, relaunchDesktop,
} from '../../support/electron/electron-fixture'
import { activeDetail, ready, send } from '../../support/runtime/llm-e2e-helpers'

const beforeUser = 'PRE_RELAUNCH_USER_MARKER'
const beforeAssistant = 'PRE_RELAUNCH_ASSISTANT_MARKER'
const afterUser = 'POST_RELAUNCH_USER_MARKER'
const afterAssistant = 'POST_RELAUNCH_ASSISTANT_MARKER'

test('retains provider history across Electron and sidecar relaunch', async () => {
  let desktop = await launchDesktop('real', 'provider-electron-relaunch')
  try {
    await ready(desktop.page)
    const initial = await runtimeState(desktop.page)
    expect(initial).toMatchObject({
      generation: 1, runtimeCount: 1, state: 'ready', sessionStatus: 'waiting',
      activeMatches: true,
    })
    const initialElectronPid = desktop.app.process().pid

    await send(desktop.page, beforeUser)
    await expect(desktop.page.getByTestId('conversation')).toContainText(
      beforeAssistant, { timeout: 20_000 },
    )
    await ready(desktop.page)
    await expectHistory(desktop.page, 1)

    desktop = await relaunchDesktop(desktop)
    await ready(desktop.page)
    expect(desktop.app.process().pid).not.toBe(initialElectronPid)
    const restored = await runtimeState(desktop.page)
    expect(restored).toMatchObject({
      sessionId: initial.sessionId, generation: 1, runtimeCount: 1,
      state: 'ready', sessionStatus: 'waiting', activeMatches: true,
      rootAgent: initial.rootAgent,
    })
    expect(restored.runtimeId).not.toBe(initial.runtimeId)
    await expect(desktop.page.locator(`[data-session-id="${initial.sessionId}"]`)).toHaveCount(1)
    await expectHistory(desktop.page, 1)

    await send(desktop.page, afterUser)
    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation).toContainText(afterAssistant, { timeout: 20_000 })
    await ready(desktop.page)
    await expectHistory(desktop.page, 2)
    expect(await runtimeState(desktop.page)).toMatchObject({
      sessionId: restored.sessionId, runtimeId: restored.runtimeId,
      generation: restored.generation, runtimeCount: 1, state: 'ready',
      sessionStatus: 'waiting', activeMatches: true,
    })

    const detail = await activeDetail(desktop.page)
    const dialogue = detail.conversation.filter((entry) => (
      entry.role === 'user' || entry.role === 'assistant'
    ))
    expect(dialogue.map(({ role, text }) => ({ role, text }))).toEqual([
      { role: 'user', text: beforeUser },
      { role: 'assistant', text: beforeAssistant },
      { role: 'user', text: afterUser },
      { role: 'assistant', text: afterAssistant },
    ])
    expect(new Set(dialogue.map(({ id }) => id)).size).toBe(dialogue.length)

    expect(await desktop.llm!.requests()).toEqual([
      expect.objectContaining({
        sequence: 1, protocol: 'anthropic', messageCount: 1,
        lastUserText: beforeUser, matched: true,
      }),
      expect.objectContaining({
        sequence: 2, protocol: 'anthropic', messageCount: 3,
        lastUserText: afterUser, assistantBlockTypes: ['text'], matched: true,
      }),
    ])
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, requestCount: 2, remaining: 0, unmatchedRequests: 0, verified: true,
    })
    await expect(desktop.page.getByLabel('Message Loopal')).toBeEnabled()
    await expect(desktop.page.getByLabel('Message Loopal')).toHaveValue('')
  } finally {
    await closeDesktop(desktop)
  }
})

async function runtimeState(page: Page) {
  return page.evaluate(async () => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    const sessionId = bootstrap.activeSessionId!
    const detail = await window.loopalDesktop.openSession(sessionId)
    const runtimes = bootstrap.runtimes.filter((runtime) => runtime.sessionId === sessionId)
    const runtime = runtimes.find(({ id }) => id === detail.session.activeRuntimeId)!
    return {
      sessionId, runtimeId: runtime.id, generation: runtime.generation,
      rootAgent: runtime.rootAgent, state: runtime.state, runtimeCount: runtimes.length,
      sessionStatus: detail.session.status,
      activeMatches: detail.session.activeRuntimeId === runtime.id,
    }
  })
}

async function expectHistory(page: Page, turns: number): Promise<void> {
  const conversation = page.getByTestId('conversation')
  await expect(conversation.locator('[data-message-role="user"]')).toHaveCount(turns)
  await expect(conversation.locator('[data-message-role="assistant"]')).toHaveCount(turns)
  await expect(conversation.locator('[data-message-role="user"]', {
    hasText: beforeUser,
  })).toHaveCount(1)
  await expect(conversation.locator('[data-message-role="assistant"]', {
    hasText: beforeAssistant,
  })).toHaveCount(1)
  if (turns === 2) {
    await expect(conversation.locator('[data-message-role="user"]', {
      hasText: afterUser,
    })).toHaveCount(1)
    await expect(conversation.locator('[data-message-role="assistant"]', {
      hasText: afterAssistant,
    })).toHaveCount(1)
  }
}
