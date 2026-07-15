import { defineConfig } from 'vitest/config'
import { resolve } from 'node:path'

export default defineConfig({
  test: {
    globals: true,
    environment: 'jsdom',
    pool: 'threads',
    maxWorkers: 2,
    testTimeout: 30_000,
    setupFiles: ['./test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}', 'test/**/*.test.{ts,tsx}'],
    clearMocks: true,
    coverage: {
      provider: 'v8',
      all: true,
      reportsDirectory: process.env.TEST_UNDECLARED_OUTPUTS_DIR
        ? resolve(process.env.TEST_UNDECLARED_OUTPUTS_DIR, 'coverage')
        : resolve(__dirname, 'coverage'),
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/**/*.d.ts',
        'src/**/*.test.ts',
        'src/**/*.test.tsx',
        'src/**/*.test-fixtures.ts',
        'src/main/index.ts',
        'src/preload/index.ts',
        'src/renderer/index.tsx',
      ],
      reporter: ['text', 'json-summary', 'html'],
      thresholds: {
        perFile: false,
        statements: 95,
        branches: 90,
        functions: 94,
        lines: 95,
      },
    },
  },
})
