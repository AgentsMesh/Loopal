import { detectRendererPlatform } from './renderer-platform'

describe('detectRendererPlatform', () => {
  it.each([
    ['MacIntel', 'darwin'],
    ['macOS', 'darwin'],
    ['Win32', 'windows'],
    ['Linux x86_64', 'linux'],
  ] as const)('maps %s to %s', (value, expected) => {
    expect(detectRendererPlatform(value)).toBe(expected)
  })
})
