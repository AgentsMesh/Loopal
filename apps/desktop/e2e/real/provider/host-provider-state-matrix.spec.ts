import { expect, test, type Page } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'
import { type E2eProvider } from '../../support/providers/provider-e2e-fixture'

interface StateCase {
  provider: E2eProvider
  scenario: string
  model: string
  prompt: string
  thinking: string
  streaming: string
  cacheCreation: number
  cacheRead: number
  thinkingTokens: number
  requestCount?: number
}

const cases: readonly StateCase[] = [
  { provider: 'anthropic', scenario: 'provider-state-anthropic', model: 'claude-opus-4-8',
    prompt: 'Observe Anthropic provider state', thinking: 'ANTHROPIC THINKING STATE MARKER',
    streaming: 'ANTHROPIC STREAMING STATE MARKER', cacheCreation: 3, cacheRead: 5,
    thinkingTokens: 13 },
  { provider: 'openai', scenario: 'provider-state-openai', model: 'o3',
    prompt: 'Observe OpenAI provider state', thinking: 'OPENAI THINKING STATE MARKER',
    streaming: 'OPENAI STREAMING STATE MARKER', cacheCreation: 0, cacheRead: 5,
    thinkingTokens: 11 },
  { provider: 'openai_compat', scenario: 'provider-state-openai-compat',
    model: 'deepseek-reasoner', prompt: 'Observe compatible provider state',
    thinking: 'COMPAT THINKING STATE MARKER', streaming: 'COMPAT STREAMING STATE MARKER',
    cacheCreation: 0, cacheRead: 0, thinkingTokens: 11 },
  { provider: 'google', scenario: 'provider-state-google',
    model: 'gemini-2.5-flash-preview-04-17', prompt: 'Observe Google provider state',
    thinking: 'GOOGLE THINKING STATE MARKER', streaming: 'GOOGLE STREAMING STATE MARKER',
    cacheCreation: 0, cacheRead: 0, thinkingTokens: 11, requestCount: 2 },
]

for (const state of cases) {
  test(`${state.provider} projects thinking, streaming, and usage telemetry`, async () => {
    const desktop = await launchDesktop('real', state.scenario, {}, state.provider)
    try {
      const page = desktop.page
      await ready(page)
      await selectThinkingModel(page, state.model)
      await send(page, state.prompt)
      const conversation = page.getByTestId('conversation')
      await expect(conversation).toContainText(state.thinking, { timeout: 15_000 })
      await expect(page.getByTestId('runtime-status')).toContainText('Thinking')
      await expect(conversation).toContainText(state.streaming, { timeout: 15_000 })
      await expect(page.getByTestId('runtime-status')).toContainText('Streaming')
      await ready(page)

      const detail = await activeDetail(page)
      const root = detail.agents.find((agent) => agent.id === 'main')!
      expect(root.view).toMatchObject({ thinkingActive: false, streamingThinking: '',
        streamingText: '', retryBanner: null })
      expect(detail.session.attention).not.toBe('failure')
      expect(root.telemetry).toMatchObject({
        turnCount: 1, inputTokens: 37, outputTokens: 13,
        cacheCreationTokens: state.cacheCreation, cacheReadTokens: state.cacheRead,
      })
      expect(root.telemetry!.thinkingTokens).toBe(state.thinkingTokens)
      const thought = root.conversation!.find((entry) => (
        entry.role === 'thinking' && entry.text.includes(state.thinking)
      ))!
      expect(thought.thinkingTokens).toBe(state.thinkingTokens)
      expect(root.conversation!.filter((entry) => entry.role === 'thinking')).toHaveLength(1)
      const requests = await desktop.llm!.requests()
      expect(requests).toHaveLength(state.requestCount ?? 1)
      expect(requests[0]).toEqual(expect.objectContaining({
        protocol: protocol(state.provider), model: state.model,
        thinkingEnabled: true, matched: true,
      }))
      if (state.provider === 'google') {
        expect(requests[1]!.assistantBlockTypes).toEqual(['thinking', 'text', 'tool_use'])
        expect(root.conversation).toEqual(expect.arrayContaining([
          expect.objectContaining({ text: 'GOOGLE SIGNATURE REPLAY MARKER' }),
        ]))
      }
      await expect.poll(() => desktop.llm!.state()).toMatchObject({
        served: state.requestCount ?? 1, remaining: 0, unmatchedRequests: 0, verified: true,
      })
    } finally {
      await closeDesktop(desktop)
    }
  })
}

async function selectThinkingModel(page: Page, model: string): Promise<void> {
  const target = await runtimeTarget(page)
  for (const command of [
    { type: 'model' as const, model },
    { type: 'thinking' as const, config: { type: 'effort' as const, level: 'high' as const } },
  ]) await page.evaluate(async ([value, control]) => {
    await window.loopalDesktop.controlAgent({ target: value, command: control })
  }, [target, command] as const)
}

function protocol(provider: E2eProvider): string {
  if (provider === 'openai') return 'openai_responses'
  return provider
}
