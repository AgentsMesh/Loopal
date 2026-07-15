import { vi } from 'vitest'
import { FakeChild } from '../host/desktop-host.test-fixtures.ts'
import { spawnDesktopProcess } from './desktop-process'

describe('DesktopProcess MetaHub startup', () => {
  it('passes address and Hub name as shell-free argv while keeping token in env', () => {
    const child = new FakeChild()
    const spawn = vi.fn((_command: string, _args: readonly string[], _options: unknown) => child)
    const result = spawnDesktopProcess(
      '/bin/loopal', '/workspace', 42, { CUSTOM: 'yes' }, undefined,
      { address: '127.0.0.1:9000', hubName: 'desktop-a', token: 'private-token' },
      spawn as never,
    )
    expect(result).toBe(child)
    const [, args, options] = spawn.mock.calls[0]!
    expect(args).toEqual([
      'desktop', 'serve', '--parent-pid', '42',
      '--join-hub', '127.0.0.1:9000', '--hub-name', 'desktop-a',
    ])
    expect(args).not.toContain('private-token')
    expect(options).toMatchObject({
      cwd: '/workspace', shell: false,
      env: expect.objectContaining({ CUSTOM: 'yes', LOOPAL_META_HUB_TOKEN: 'private-token' }),
    })
  })
})
