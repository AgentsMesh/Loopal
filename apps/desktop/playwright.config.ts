import { defineConfig } from '@playwright/test'
import { join } from 'node:path'

const artifacts = process.env.TEST_UNDECLARED_OUTPUTS_DIR
  ? join(process.env.TEST_UNDECLARED_OUTPUTS_DIR, 'playwright')
  : 'test-results'

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  workers: 1,
  timeout: 120_000,
  expect: { timeout: 10_000 },
  retries: process.env.CI ? 1 : 0,
  outputDir: artifacts,
  reporter: [['line']],
  use: {
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
})
