import { expect, test, type Locator } from '@playwright/test'
import { readFile, writeFile } from 'node:fs/promises'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'

test('returns reject, edited approval, and direct approval to the real model loop', async () => {
  const desktop = await launchDesktop('real', 'provider-plan-approval')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    const target = await runtimeTarget(page)
    await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)
    await expect.poll(async () => {
      const detail = await activeDetail(page)
      return detail.agents.find((agent) => agent.id === target.agentId)?.permissionMode
    }).toBe('bypass')
    await send(page, 'Enter deterministic plan mode')
    const permissions = page.getByTestId('permissions-pane')
    await expect(permissions).toContainText('Allow EnterPlanMode')
    await permissions.getByRole('button', { name: 'Allow for session' }).click()
    await expect(conversation).toContainText('Plan mode is ready for fixture seeding.', {
      timeout: 20_000,
    })
    const planPath = await planFilePath(conversation, 'EnterPlanMode')
    await writeFile(planPath, '# ORIGINAL PLAN MARKER\n')

    await send(page, 'Request a rejected plan review')
    let card = page.getByTestId('plan-approval-card')
    await expect(card).toContainText('# ORIGINAL PLAN MARKER', { timeout: 20_000 })
    await card.getByTestId('plan-approval-reject').click()
    await expect(conversation).toContainText(
      'The rejected plan returned to the model for revision.', { timeout: 20_000 },
    )
    await expect(card).toHaveCount(0)
    expect(await readFile(planPath, 'utf8')).toBe('# ORIGINAL PLAN MARKER\n')

    await send(page, 'Request an edited plan approval')
    card = page.getByTestId('plan-approval-card')
    await expect(card).toBeVisible({ timeout: 20_000 })
    await card.getByTestId('plan-approval-editor').fill('# EDITED PLAN MARKER\n')
    await card.getByTestId('plan-approval-approve-edits').click()
    await expect(conversation).toContainText(
      'The edited approval returned to the model and restored Act mode.', { timeout: 20_000 },
    )
    await expect(card).toHaveCount(0)
    expect(await readFile(planPath, 'utf8')).toBe('# EDITED PLAN MARKER\n')
    expect((await activeDetail(page)).agents.find((agent) => agent.id === 'main')?.mode).toBe('act')

    await send(page, 'Enter plan mode for direct approval')
    await expect(conversation).toContainText('Second plan review is ready.', { timeout: 20_000 })
    const secondPath = await planFilePath(conversation, 'EnterPlanMode')
    await writeFile(secondPath, '# DIRECT PLAN MARKER\n')
    await send(page, 'Request direct plan approval')
    card = page.getByTestId('plan-approval-card')
    await expect(card).toContainText('# DIRECT PLAN MARKER', { timeout: 20_000 })
    await card.getByTestId('plan-approval-approve').click()
    await expect(conversation).toContainText(
      'The direct approval returned to the model.', { timeout: 20_000 },
    )
    await expect(card).toHaveCount(0)
    expect(await readFile(secondPath, 'utf8')).toBe('# DIRECT PLAN MARKER\n')
    expect((await activeDetail(page)).agents.find((agent) => agent.id === 'main')?.mode).toBe('act')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 10, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

async function planFilePath(conversation: Locator, toolName: string): Promise<string> {
  const tool = conversation.getByTestId('tool-invocation').filter({ hasText: toolName }).last()
  await expect(tool.getByLabel('Completed')).toBeVisible({ timeout: 20_000 })
  await tool.locator(':scope > summary').click()
  const match = (await tool.textContent())?.match(/(\/\S+\/\.loopal\/plans\/\S+\.md)/u)
  if (!match?.[1]) throw new Error('EnterPlanMode did not expose its plan file path')
  return match[1]
}
