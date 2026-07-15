import { execFile as execFileCallback } from 'node:child_process'
import { promisify } from 'node:util'
import { expect, test } from '@playwright/test'
import {
  closeDesktop, launchDesktop, relaunchDesktop, waitForHostStatus,
} from '../../../support/electron/electron-fixture'

const execFile = promisify(execFileCallback)

test('manages a real local MetaHub and rejoins it through the sidecar', async () => {
  const desktop = await launchDesktop('real')
  try {
    const result = await desktop.page.evaluate(async () => {
      const api = window.loopalDesktop
      const bootstrap = await api.bootstrap()
      const sessionId = bootstrap.activeSessionId!
      const runtime = bootstrap.runtimes.find((item) => item.sessionId === sessionId)!
      const target = { sessionId, runtimeId: runtime.id, generation: runtime.generation }
      const local = await api.startLocalMetaHub({ bindAddress: '127.0.0.1:0' })
      const settings = await api.getMetaHubSettings()
      const joined = await api.joinMetaHub(target)
      const detail = await api.openSession(sessionId)
      const disconnected = await api.disconnectMetaHub(target)
      const rejoined = await api.joinMetaHub(target)
      await api.disconnectMetaHub(target)
      const stopped = await api.stopLocalMetaHub()
      return { local, settings, joined, projected: detail.metaHub, disconnected, rejoined, stopped }
    })
    expect(result.local).toMatchObject({ state: 'running', address: expect.any(String) })
    expect(result.settings).toMatchObject({
      tokenConfigured: true,
      address: result.local.address,
    })
    expect(Object.keys(result.settings)).not.toContain('token')
    expect(result.joined).toMatchObject({
      state: 'connected',
      hubs: [expect.objectContaining({ status: 'connected', agentCount: 1 })],
    })
    expect(result.joined.topology).toEqual(expect.arrayContaining([
      expect.objectContaining({ name: 'main', lifecycle: 'running' }),
    ]))
    expect(result.projected?.state).toBe('connected')
    expect(result.disconnected).toMatchObject({ state: 'disconnected', hubs: [] })
    expect(result.rejoined.state).toBe('connected')
    expect(result.stopped).toEqual({ state: 'stopped' })
  } finally {
    await closeDesktop(desktop)
  }
})

test('clears a crashed managed MetaHub credential before relaunch', async () => {
  test.skip(process.platform === 'win32', 'process tree fixture uses ps')
  let desktop = await launchDesktop('real')
  try {
    await desktop.page.evaluate(async () => {
      const api = window.loopalDesktop
      const local = await api.startLocalMetaHub({ bindAddress: '127.0.0.1:0' })
      if (local.state !== 'running' || !local.address) {
        throw new Error('local MetaHub did not start')
      }
      const settings = await api.getMetaHubSettings()
      await api.updateMetaHubSettings({
        address: local.address,
        hubName: settings.hubName,
        joinOnStart: true,
        startLocalOnLaunch: false,
      })
    })
    const parentPid = desktop.app.process().pid!
    let managedPid: number | undefined
    await expect.poll(async () => {
      managedPid = await localMetaHubPid(parentPid)
      return managedPid
    }, { timeout: 10_000 }).not.toBeUndefined()
    process.kill(managedPid!, 'SIGKILL')
    await expect.poll(async () => desktop.page.evaluate(async () => (
      await window.loopalDesktop.getLocalMetaHubStatus()
    ).state), { timeout: 10_000 }).toBe('failed')

    const stopped = await desktop.page.evaluate(async () => {
      await window.loopalDesktop.stopLocalMetaHub()
      return window.loopalDesktop.getMetaHubSettings()
    })
    expect(stopped).toMatchObject({
      address: '', joinOnStart: false, tokenConfigured: false,
    })

    desktop = await relaunchDesktop(desktop)
    await waitForHostStatus(desktop.page, 'ready')
    const relaunched = await desktop.page.evaluate(async () => ({
      settings: await window.loopalDesktop.getMetaHubSettings(),
      bootstrap: await window.loopalDesktop.bootstrap(),
    }))
    expect(relaunched.settings).toMatchObject({
      address: '', joinOnStart: false, tokenConfigured: false,
    })
    expect(relaunched.bootstrap.runtimes).toContainEqual(
      expect.objectContaining({ state: 'ready' }),
    )
  } finally {
    await closeDesktop(desktop)
  }
})

test('starts a managed MetaHub without implicitly joining a session after relaunch', async () => {
  let desktop = await launchDesktop('real')
  try {
    await desktop.page.evaluate(async () => {
      const current = await window.loopalDesktop.getMetaHubSettings()
      await window.loopalDesktop.updateMetaHubSettings({
        address: '', hubName: current.hubName,
        joinOnStart: false, startLocalOnLaunch: true,
      })
    })
    desktop = await relaunchDesktop(desktop)
    await waitForHostStatus(desktop.page, 'ready')
    const readState = async () => desktop.page.evaluate(async () => {
      const api = window.loopalDesktop
      const local = await api.getLocalMetaHubStatus()
      const settings = await api.getMetaHubSettings()
      const bootstrap = await api.bootstrap()
      const sessionId = bootstrap.activeSessionId!
      const detail = await api.openSession(sessionId)
      return { local, settings, connected: detail.metaHub?.state }
    })
    await expect.poll(readState, { timeout: 20_000 }).toMatchObject({
      local: { state: 'running', address: expect.any(String) },
      settings: {
        startLocalOnLaunch: true, tokenConfigured: true,
        address: expect.any(String),
      },
      connected: 'disconnected',
    })
    const state = await readState()
    expect(state.settings.address).toBe(state.local.address)
    expect(state.settings).not.toHaveProperty('token')
  } finally {
    await closeDesktop(desktop)
  }
})

async function localMetaHubPid(parentPid: number): Promise<number | undefined> {
  const { stdout } = await execFile('ps', ['-axo', 'pid=,ppid=,command='])
  for (const line of stdout.split('\n')) {
    const match = /^\s*(\d+)\s+(\d+)\s+(.+)$/.exec(line)
    if (match && Number(match[2]) === parentPid && match[3]!.includes(' --meta-hub ')) {
      return Number(match[1])
    }
  }
  return undefined
}
