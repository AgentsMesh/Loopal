import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../support/electron/electron-fixture'
import { selectSettingsSection } from '../support/settings/settings-helpers'

test('adds, edits, disables, reenables, and deletes typed MCP definitions', async () => {
  const desktop = await launchDesktop('fake')
  try {
    const page = desktop.page
    const workspaceId = await page.evaluate(async () => (
      await window.loopalDesktop.bootstrap()
    ).workspaces[0]!.id)
    await page.getByRole('button', { name: 'Settings' }).click()
    await selectSettingsSection(page, 'mcp')
    const section = page.getByTestId('loopal-mcp-settings')
    await expect(section.getByRole('heading', {
      name: 'Loopal MCP servers (new/restarted Sessions)',
    })).toBeVisible()
    await section.getByRole('button', { name: 'Add MCP server' }).click()
    await section.getByLabel('MCP server name').fill('local-tools')
    await section.getByLabel('MCP command').fill('node')
    await section.getByLabel('MCP arguments').fill('server.js\n--stdio')
    await section.getByLabel('MCP sharing').selectOption('per-agent')
    await section.getByLabel('MCP timeout milliseconds').fill('12000')
    await section.getByLabel('Use MCP cwd isolation').check()
    await section.getByLabel('Cwd isolation cache subdirectory').fill('local-tools')
    await section.getByLabel('New env name').fill('TOKEN')
    await section.getByLabel('New env value').fill('fake-env-secret')
    await section.getByRole('button', { name: 'Set secret' }).click()
    await section.getByRole('button', { name: 'Save MCP server' }).click()
    await expect(section.getByRole('status')).toContainText('Restart Sessions')
    const stdioCard = section.locator('.mcp-server-card').filter({ hasText: 'local-tools' })
    await expect(stdioCard).toContainText('Enabled')
    const listed = await page.evaluate(
      (id) => window.loopalDesktop.listMcpServers(id), workspaceId,
    )
    expect(listed.servers[0]).toMatchObject({
      name: 'local-tools', type: 'stdio', env: [{ name: 'TOKEN', configured: true }],
    })
    expect(JSON.stringify(listed)).not.toContain('fake-env-secret')
    expect(await page.content()).not.toContain('fake-env-secret')

    await stdioCard.getByRole('button', { name: 'Edit' }).click()
    await expect(section.getByLabel('Secret value TOKEN')).toHaveValue('')
    await expect(section).toContainText('configured · preserved')
    await section.getByLabel('Enable MCP server').uncheck()
    await section.getByLabel('Secret value TOKEN').fill('fake-replacement-secret')
    await section.getByRole('button', { name: 'Save MCP server' }).click()
    await expect(stdioCard).toContainText('Disabled')
    expect(await page.content()).not.toContain('fake-replacement-secret')
    await stdioCard.getByRole('button', { name: 'Edit' }).click()
    await section.getByLabel('Enable MCP server').check()
    await section.getByRole('button', { name: 'Save MCP server' }).click()
    await expect(stdioCard).toContainText('Enabled')

    await section.getByRole('button', { name: 'Add MCP server' }).click()
    await section.getByLabel('MCP transport').selectOption('streamable-http')
    await section.getByLabel('MCP server name').fill('remote-tools')
    await section.getByLabel('MCP HTTP URL').fill('https://mcp.example.test/api')
    await section.getByLabel('New header name').fill('Authorization')
    await section.getByLabel('New header value').fill('fake-header-secret')
    await section.getByRole('button', { name: 'Set secret' }).click()
    await section.getByRole('button', { name: 'Save MCP server' }).click()
    const httpCard = section.locator('.mcp-server-card').filter({ hasText: 'remote-tools' })
    await expect(httpCard).toContainText('streamable-http')
    const finalList = await page.evaluate(
      (id) => window.loopalDesktop.listMcpServers(id), workspaceId,
    )
    expect(JSON.stringify(finalList)).not.toContain('fake-header-secret')
    expect(finalList.servers.find((server) => server.name === 'remote-tools')).toMatchObject({
      headers: [{ name: 'Authorization', configured: true }],
    })
    await httpCard.getByRole('button', { name: 'Delete MCP server remote-tools' }).click()
    await stdioCard.getByRole('button', { name: 'Delete MCP server local-tools' }).click()
    await expect(section.locator('.mcp-server-card')).toHaveCount(0)
  } finally {
    await closeDesktop(desktop)
  }
})
