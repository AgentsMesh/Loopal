import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, send } from '../../support/runtime/llm-e2e-helpers'

test('renders retry, fatal, recovery, and retry-exhaustion HTTP states', async () => {
  const desktop = await launchDesktop('real', 'provider-http-errors')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)

    await send(page, 'Recover one server error')
    await expect(conversation).toContainText(/Retrying in \d+\.\ds/, { timeout: 10_000 })
    await expect(conversation).toContainText('Recovered after one HTTP 503.', {
      timeout: 20_000,
    })
    await expect(conversation).not.toContainText('Retrying in')
    await ready(page)

    await send(page, 'Exercise fatal authentication')
    await expect(conversation.locator('[data-message-role="error"]')).toContainText(
      'status=401', { timeout: 20_000 },
    )
    await expect(page.getByTestId('runtime-status')).toContainText('Failed')
    await expect(page.getByLabel('Message Loopal')).toBeEnabled()
    const diagnosticsTab = page.getByRole('tab', { name: 'Diagnostics', exact: true })
    await diagnosticsTab.click()
    await expect(page.getByTestId('diagnostics-pane')).toContainText('status=401')

    await send(page, 'Recover after fatal provider error')
    await expect(conversation).toContainText(
      'Session recovered after a fatal provider turn.', { timeout: 20_000 },
    )
    await ready(page)

    await send(page, 'Exhaust provider retries')
    await expect(conversation.locator('[data-message-role="error"]').last()).toContainText(
      'retry after 0ms', { timeout: 20_000 },
    )
    await expect(page.getByTestId('runtime-status')).toContainText('Failed')
    expect(await desktop.llm!.requests()).toHaveLength(11)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 11, remaining: 0, unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('keeps malformed, empty, and thinking-only stream failures recoverable', async () => {
  const desktop = await launchDesktop('real', 'provider-stream-faults')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)

    await send(page, 'Recover malformed partial stream')
    await expect(conversation).toContainText('SSE parse error', { timeout: 20_000 })
    await expect(conversation).toContainText(
      'Malformed SSE recovered through continuation.', { timeout: 20_000 },
    )
    await expect(conversation).toContainText(
      'Response stream ended unexpectedly. Auto-continuing (1/3)',
    )
    await ready(page)

    await send(page, 'Handle empty stream disconnect')
    await expect(conversation).toContainText(/Retrying in \d+\.\ds/, { timeout: 10_000 })
    await expect(conversation).toContainText(
      'Empty stream recovered through exact request retry.', { timeout: 20_000 },
    )
    await expect(conversation).not.toContainText('possible network interruption')
    await expect(conversation).not.toContainText('Retrying in')
    await ready(page)

    await send(page, 'Recover thinking-only disconnect')
    await expect(conversation).toContainText('THINKING BEFORE DISCONNECT', { timeout: 20_000 })
    await expect(conversation).toContainText(
      'Thinking-only disconnect recovered.', { timeout: 20_000 },
    )
    await ready(page)
    await send(page, 'Recover server-only disconnect')
    await expect(conversation).toContainText('PARTIAL SERVER RESULT', { timeout: 20_000 })
    await expect(conversation).toContainText(
      'Server-only disconnect recovered.', { timeout: 20_000 },
    )
    await ready(page)

    await send(page, 'Continue after all stream recoveries')
    await expect(conversation).toContainText(
      'Session remained usable after every stream recovery.', { timeout: 20_000 },
    )
    await ready(page)

    const detail = await activeDetail(page)
    expect(detail.session.attention).not.toBe('failure')
    expect(detail.agents.find((agent) => agent.id === 'main')?.telemetry?.turnCount).toBe(5)
    expect(detail.conversation.some((entry) => (
      entry.role === 'thinking' && entry.text.includes('THINKING BEFORE DISCONNECT')
    ))).toBe(true)
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(9)
    expect(requests[3]).toEqual({ ...requests[2], sequence: requests[3]!.sequence })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 9, remaining: 0, unmatchedRequests: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('force-compacts and retries a real context overflow', async () => {
  const desktop = await launchDesktop('real', 'provider-context-recovery')
  try {
    const page = desktop.page
    const conversation = page.getByTestId('conversation')
    await ready(page)
    for (const message of ['Seed recovery context one', 'Seed recovery context two']) {
      await send(page, message)
      await ready(page)
    }

    await send(page, 'Trigger context overflow recovery')
    await expect(page.getByTestId('runtime-status')).toContainText('Compacting', {
      timeout: 20_000,
    })
    await expect(conversation).toContainText(
      'Context overflow — compacting and retrying...', { timeout: 20_000 },
    )
    await expect(conversation).toContainText(
      'Context overflow recovered after real compaction.', { timeout: 30_000 },
    )
    await expect(conversation).toContainText('Context compacted (context_overflow)')
    await ready(page)
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(5)
    expect(requests[3]!.model).toContain('claude')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 5, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})

test('condenses rejected server blocks before retrying', async () => {
  const desktop = await launchDesktop('real', 'provider-server-block-recovery')
  try {
    const page = desktop.page
    await ready(page)
    await send(page, 'Trigger server block recovery')
    await expect(page.getByTestId('conversation')).toContainText(
      'Server blocks condensed and the request recovered.', { timeout: 20_000 },
    )
    await ready(page)
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(3)
    expect(requests[1]!.serverBlockCount).toBeGreaterThanOrEqual(2)
    expect(requests[2]!.serverBlockCount).toBe(0)
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 3, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
