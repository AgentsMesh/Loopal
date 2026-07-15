import { expect, test } from '@playwright/test'
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import {
  closeDesktop, launchDesktop, waitForHostStatus,
} from '../../support/electron/electron-fixture'
import { selectSettingsSection } from '../../support/settings/settings-helpers'

test('persists redacted typed MCP definitions through the real Rust Host', async () => {
  const desktop = await launchDesktop('real')
  try {
    const page = desktop.page
    await waitForHostStatus(page, 'ready')
    const directory = join(desktop.project, '.loopal')
    const path = join(directory, 'settings.local.json')
    await mkdir(directory, { recursive: true })
    await writeFile(path, JSON.stringify({
      unknown_root: { keep: true },
      mcp_servers: {
        local_tools: {
          type: 'stdio', command: 'before', args: [], enabled: true, timeout_ms: 30_000,
          env: { KEEP: 'preserved-secret', REMOVE: 'removed-secret' },
          future_field: { keep: true },
        },
        other: {
          type: 'streamable-http', url: 'https://other.example.test/mcp', enabled: true,
          headers: { Authorization: 'other-secret' },
        },
      },
    }))
    const before = await page.evaluate(
      () => window.loopalDesktop.listMcpServers('local-workspace'),
    )
    expect(before.servers.find((server) => server.name === 'local_tools')).toMatchObject({
      env: [{ name: 'KEEP', configured: true }, { name: 'REMOVE', configured: true }],
    })
    expect(JSON.stringify(before)).not.toMatch(/preserved-secret|removed-secret|other-secret/)

    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'mcp')
    await expect(page.getByTestId('loopal-mcp-settings')).toContainText('local_tools')
    const updated = await page.evaluate(() => window.loopalDesktop.upsertMcpServer({
      workspaceId: 'local-workspace', server: {
        type: 'stdio', name: 'local_tools', command: 'node', args: ['server.js'],
        enabled: true, timeoutMs: 12_000, sharing: 'spawn-tree',
        cwdIsolation: { arg: '--user-data-dir', cacheSubdir: 'local-tools' },
        secretPatches: [
          { target: 'env', name: 'TOKEN', operation: 'set', value: 'new-env-secret' },
          { target: 'env', name: 'REMOVE', operation: 'remove' },
        ],
      },
    }))
    expect(JSON.stringify(updated)).not.toMatch(/new-env-secret|preserved-secret/)
    let raw = JSON.parse(await readFile(path, 'utf8'))
    expect(raw.unknown_root.keep).toBe(true)
    expect(raw.mcp_servers.local_tools.env).toEqual({
      KEEP: 'preserved-secret', TOKEN: 'new-env-secret',
    })
    expect(raw.mcp_servers.local_tools.future_field.keep).toBe(true)
    expect(raw.mcp_servers.other.headers.Authorization).toBe('other-secret')

    const disabled = await page.evaluate(() => window.loopalDesktop.upsertMcpServer({
      workspaceId: 'local-workspace', server: {
        type: 'stdio', name: 'local_tools', command: 'node', args: ['server.js'],
        enabled: false, timeoutMs: 12_000, sharing: 'spawn-tree', cwdIsolation: null,
        secretPatches: [],
      },
    }))
    expect(disabled.servers.find((server) => server.name === 'local_tools')?.enabled).toBe(false)
    const reenabled = await page.evaluate(() => window.loopalDesktop.upsertMcpServer({
      workspaceId: 'local-workspace', server: {
        type: 'stdio', name: 'local_tools', command: 'node', args: ['server.js'],
        enabled: true, timeoutMs: 12_000, sharing: 'spawn-tree', cwdIsolation: null,
        secretPatches: [],
      },
    }))
    expect(reenabled.servers.find((server) => server.name === 'local_tools')?.enabled).toBe(true)
    raw = JSON.parse(await readFile(path, 'utf8'))
    expect(raw.mcp_servers.local_tools.env.KEEP).toBe('preserved-secret')
    expect(raw.mcp_servers.local_tools.env.TOKEN).toBe('new-env-secret')

    const withHttp = await page.evaluate(() => window.loopalDesktop.upsertMcpServer({
      workspaceId: 'local-workspace', server: {
        type: 'streamable-http', name: 'remote_tools', url: 'https://mcp.example.test/api',
        enabled: true, timeoutMs: 15_000, sharing: 'hub-singleton', secretPatches: [{
          target: 'header', name: 'Authorization', operation: 'set', value: 'Bearer new-header-secret',
        }],
      },
    }))
    expect(JSON.stringify(withHttp)).not.toContain('new-header-secret')
    raw = JSON.parse(await readFile(path, 'utf8'))
    expect(raw.mcp_servers.remote_tools.headers.Authorization).toBe('Bearer new-header-secret')

    const bootstrap = await page.evaluate(() => window.loopalDesktop.bootstrap())
    await page.evaluate((sessionId) => window.loopalDesktop.restartSession(sessionId),
      bootstrap.activeSessionId!)
    const afterRestart = await page.evaluate(
      () => window.loopalDesktop.listMcpServers('local-workspace'),
    )
    expect(afterRestart.servers.map((server) => server.name)).toEqual(expect.arrayContaining([
      'local_tools', 'remote_tools',
    ]))
    await page.evaluate(() => window.loopalDesktop.deleteMcpServer({
      workspaceId: 'local-workspace', name: 'local_tools',
    }))
    const deleted = await page.evaluate(() => window.loopalDesktop.deleteMcpServer({
      workspaceId: 'local-workspace', name: 'remote_tools',
    }))
    expect(deleted.servers.some((server) => ['local_tools', 'remote_tools'].includes(server.name)))
      .toBe(false)
    raw = JSON.parse(await readFile(path, 'utf8'))
    expect(raw.mcp_servers.local_tools.enabled).toBe(false)
    expect(raw.mcp_servers.local_tools.env).toBeUndefined()
    expect(raw.mcp_servers.remote_tools.headers).toBeUndefined()
    expect(raw.mcp_servers.other.headers.Authorization).toBe('other-secret')
    await expect(page.evaluate(
      () => window.loopalDesktop.listMcpServers('outside-workspace'),
    )).rejects.toThrow(/Unknown workspace|unknown workspace/)
  } finally {
    await closeDesktop(desktop)
  }
})
