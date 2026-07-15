import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  isolatedTestEnvironment, type MockLlmFixture,
} from './mock-llm-fixture'

export type E2eProvider = 'anthropic' | 'openai' | 'openai_compat' | 'google'

export function providerEnvironment(
  llm: MockLlmFixture, provider: E2eProvider,
): Record<string, string> {
  const common = { LOOPAL_OTEL_ENABLED: 'false' }
  if (provider === 'anthropic') return {
    ...common, ANTHROPIC_API_KEY: llm.apiKey, ANTHROPIC_BASE_URL: llm.baseUrl,
  }
  if (provider === 'openai') return {
    ...common, OPENAI_API_KEY: llm.apiKey, OPENAI_BASE_URL: `${llm.baseUrl}/v1`,
  }
  if (provider === 'google') return {
    ...common, GOOGLE_API_KEY: llm.apiKey, GOOGLE_BASE_URL: llm.baseUrl,
  }
  return { ...common, LOOPAL_MOCK_LLM_API_KEY: llm.apiKey }
}

export async function configureProvider(
  home: string, llm: MockLlmFixture, provider: E2eProvider,
  initialSettings?: string,
): Promise<void> {
  const settings = initialSettings
    ? JSON.parse(initialSettings) as Record<string, unknown> : {}
  if (provider !== 'anthropic') settings.model = provider === 'openai' ? 'gpt-4.1'
    : provider === 'google' ? 'gemini-2.0-flash' : 'deepseek-reasoner'
  if (provider === 'openai_compat') settings.providers = {
    openai_compat: [{
      name: 'openai_compat', base_url: `${llm.baseUrl}/v1`,
      api_key_env: 'LOOPAL_MOCK_LLM_API_KEY',
    }],
  }
  if (!Object.keys(settings).length) return
  const directory = join(home, '.loopal')
  await mkdir(directory, { recursive: true })
  await writeFile(join(directory, 'settings.json'), JSON.stringify(settings))
}

export function persistedDesktopEnvironment(
  backend: 'fake' | 'real', home: string, project: string, binary: string,
  llm?: MockLlmFixture, provider: E2eProvider = 'anthropic',
): Record<string, string> {
  const env = isolatedTestEnvironment({
    HOME: home, LOOPAL_DESKTOP_CWD: project, LOOPAL_DESKTOP_E2E_HIDDEN: '1',
  })
  if (backend === 'fake') return { ...env, LOOPAL_DESKTOP_BACKEND: 'fake' }
  if (!llm) throw new Error('Real Desktop fixture requires Mock LLM')
  return {
    ...env, ...providerEnvironment(llm, provider),
    LOOPAL_DESKTOP_BINARY_RUNFILE: binary, LOOPAL_MCP_STARTUP_WAIT_SECS: '1',
  }
}

export function providerModel(provider: E2eProvider): string {
  if (provider === 'openai') return 'gpt-4.1'
  if (provider === 'google') return 'gemini-2.0-flash'
  if (provider === 'openai_compat') return 'deepseek-reasoner'
  return 'claude-opus-4-8'
}
