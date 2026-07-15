import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, send } from '../../support/runtime/llm-e2e-helpers'
import { type E2eProvider, providerModel } from '../../support/providers/provider-e2e-fixture'

const cases: readonly SearchCase[] = [{
  provider: 'openai', protocol: 'openai_responses', scenario: 'provider-server-search-openai',
  prompt: 'Use native OpenAI server search',
  answer: 'OpenAI native search rendered.', followUpAnswer: 'OpenAI search history remained ordered.',
  tool: 'web_search', result: '"status":"completed"', declaration: 'web_search',
  blocks: ['reasoning', 'web_search_call', 'message'], serverBlockCount: 1,
}, {
  provider: 'google', protocol: 'google', scenario: 'provider-server-search-google',
  prompt: 'Use native Google server search',
  answer: 'Google native search rendered.', followUpAnswer: 'Google search history remained ordered.',
  tool: 'google_search', result: 'Fixture Google source', declaration: 'WebSearch',
  blocks: ['server_tool_use', 'server_tool_result', 'text'], serverBlockCount: 2,
}]

for (const item of cases) {
  test(`${item.provider} renders and replays native server search`, async () => {
    const desktop = await launchDesktop('real', item.scenario, {}, item.provider)
    try {
      const page = desktop.page
      const conversation = page.getByTestId('conversation')
      await ready(page)
      await send(page, item.prompt)
      await expect(conversation).toContainText(item.answer, { timeout: 20_000 })
      const search = conversation.getByTestId('tool-invocation')
        .filter({ hasText: item.tool }).last()
      await expect(search.getByLabel('Completed')).toBeVisible()
      await search.locator(':scope > summary').click()
      await expect(search).toContainText(item.result)
      await expect(conversation).toContainText('Output truncated (max_tokens). Auto-continuing')
      await expect(conversation).toContainText(item.followUpAnswer, { timeout: 20_000 })
      await ready(page)
      const requests = await desktop.llm!.requests()
      expect(requests).toHaveLength(2)
      expect(requests[0]).toMatchObject({
        protocol: item.protocol, model: providerModel(item.provider),
        serverBlockCount: 0, apiKeyPresent: true, matched: true,
      })
      expect(requests[0]!.toolNames).toContain(item.declaration)
      expect(requests[1]).toMatchObject({
        protocol: item.protocol, model: providerModel(item.provider), messageCount: 2,
        lastUserText: item.prompt, assistantBlockTypes: item.blocks,
        serverBlockCount: item.serverBlockCount, matched: true,
      })
      await expect.poll(() => desktop.llm!.state()).toMatchObject({
        served: 2, remaining: 0, verified: true,
      })
    } finally {
      await closeDesktop(desktop)
    }
  })
}

interface SearchCase {
  readonly provider: Extract<E2eProvider, 'openai' | 'google'>
  readonly protocol: string
  readonly scenario: string
  readonly prompt: string
  readonly answer: string
  readonly followUpAnswer: string
  readonly tool: string
  readonly result: string
  readonly declaration: string
  readonly blocks: readonly string[]
  readonly serverBlockCount: number
}
