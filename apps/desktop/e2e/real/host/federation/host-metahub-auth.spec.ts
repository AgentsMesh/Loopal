import { readdir, readFile } from 'node:fs/promises'
import { join } from 'node:path'
import { expect, test } from '@playwright/test'
import { closeDesktop, launchDesktop } from '../../../support/electron/electron-fixture'
import {
  startMetaHub, stopProcess,
} from '../../../support/federation/metahub-data-plane-fixture'

test('rejects a bad MetaHub token without leaking it and recovers in place', async () => {
  const meta = await startMetaHub()
  const desktop = await launchDesktop('real')
  const rejectedToken = `rejected-${Date.now()}-secret`
  try {
    const result = await desktop.page.evaluate(async ({ address, bad, good }) => {
      const api = window.loopalDesktop
      const bootstrap = await api.bootstrap()
      const sessionId = bootstrap.activeSessionId!
      const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)!
      const target = {
        sessionId, runtimeId: runtime.id, generation: runtime.generation,
        agentId: runtime.rootAgent,
      }
      await api.updateMetaHubSettings({
        address, hubName: 'hub-auth', token: bad,
        joinOnStart: false, startLocalOnLaunch: false,
      })
      let failure = ''
      try { await api.joinMetaHub(target) } catch (error) { failure = String(error) }
      const rejectedSettings = await api.getMetaHubSettings()
      await api.updateMetaHubSettings({
        address, hubName: 'hub-auth', token: good,
        joinOnStart: false, startLocalOnLaunch: false,
      })
      const joined = await api.joinMetaHub(target)
      return { failure, rejectedSettings, joined }
    }, { address: meta.address, bad: rejectedToken, good: meta.token })

    expect(result.failure.toLowerCase()).toContain('token')
    expect(result.failure).not.toContain(rejectedToken)
    expect(result.rejectedSettings).toMatchObject({ tokenConfigured: true })
    expect(result.rejectedSettings).not.toHaveProperty('token')
    expect(result.joined).toMatchObject({ state: 'connected', hubName: 'hub-auth' })
    expect(await filesContaining(desktop.root, rejectedToken)).toEqual([])
  } finally {
    await closeDesktop(desktop)
    await stopProcess(meta.child)
  }
})

async function filesContaining(root: string, marker: string): Promise<string[]> {
  const matches: string[] = []
  async function visit(path: string): Promise<void> {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = join(path, entry.name)
      if (entry.isDirectory()) await visit(child)
      else if (entry.isFile()) {
        const contents = await readFile(child).catch(() => Buffer.alloc(0))
        if (contents.includes(Buffer.from(marker))) matches.push(child)
      }
    }
  }
  await visit(root)
  return matches
}
