import { expect, test, type Page } from '@playwright/test'
import { execFile as execFileCallback } from 'node:child_process'
import { basename } from 'node:path'
import { promisify } from 'node:util'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'

const execFile = promisify(execFileCallback)

test('isolates child processes and cleans active children across lifecycle controls', async () => {
  test.skip(process.platform === 'win32', 'safe process-tree probe currently uses ps')
  const desktop = await launchDesktop('real', 'subagent-process-lifecycle')
  try {
    const page = desktop.page
    const electronPid = desktop.app.process().pid!
    await ready(page)
    let target = await runtimeTarget(page)
    await bypass(page, target)
    const initial = await loopalTree(electronPid)
    const host = initial.find((row) => row.ppid === electronPid)
    expect(host).toBeDefined()
    expect(initial.some((row) => row.ppid === host!.pid)).toBe(true)

    await send(page, 'Spawn the interruptible process child')
    await expectAgent(page, 'interrupt-child', 'INTERRUPT CHILD STREAM ACTIVE')
    const interrupted = await newChildProcess(electronPid, initial, host!.pid)
    await page.evaluate(async ({ root, agentId }) => {
      await window.loopalDesktop.interruptAgent({ ...root, agentId })
    }, { root: target, agentId: 'interrupt-child' })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({ clientDisconnects: 1 })
    await expect.poll(() => agentStatus(page, 'interrupt-child')).toBe('failed')
    await expect.poll(() => agentText(page, 'interrupt-child')).toContain('Turn cancelled')
    await expect.poll(() => rootText(page)).toContain('Root observed interrupted child completion.')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({ served: 4, inFlight: 0 })
    expect([host!.pid, undefined]).toContain(await processParent(interrupted.pid))

    await send(page, 'Spawn the stop cleanup child')
    await expectAgent(page, 'stop-child', 'STOP CHILD STREAM ACTIVE')
    await expect.poll(() => rootText(page)).toContain('Stop child is running.')
    const stopTree = await loopalTree(electronPid)
    expect(stopTree.length).toBeGreaterThan(initial.length)
    await page.evaluate((sessionId) => window.loopalDesktop.stopSession(sessionId), target.sessionId)
    await expect.poll(() => livePids(stopTree)).toEqual([])
    await expect.poll(() => desktop.llm!.state()).toMatchObject({ clientDisconnects: 2, inFlight: 0 })

    const resumed = await page.evaluate(
      (sessionId) => window.loopalDesktop.restartSession(sessionId), target.sessionId,
    )
    expect(resumed.generation).toBeGreaterThan(target.generation)
    await ready(page)
    target = await runtimeTarget(page)
    const resumedTree = await loopalTree(electronPid)
    expect(resumedTree.length).toBeGreaterThanOrEqual(initial.length)
    expect(resumedTree.some((row) => stopTree.some((old) => old.pid === row.pid))).toBe(false)
    await send(page, 'Recover after stopping the Session')
    await expect.poll(() => rootText(page)).toContain('Session recovered after stop cleanup.')

    await send(page, 'Spawn the restart cleanup child')
    await expectAgent(page, 'restart-child', 'RESTART CHILD STREAM ACTIVE')
    await expect.poll(() => rootText(page)).toContain('Restart child is running.')
    const restartTree = await loopalTree(electronPid)
    const restarted = await page.evaluate(
      (sessionId) => window.loopalDesktop.restartSession(sessionId), target.sessionId,
    )
    expect(restarted.generation).toBeGreaterThan(target.generation)
    await expect.poll(() => livePids(restartTree)).toEqual([])
    await expect.poll(() => desktop.llm!.state()).toMatchObject({ clientDisconnects: 3, inFlight: 0 })
    await ready(page)
    const finalTree = await loopalTree(electronPid)
    expect(finalTree.length).toBeGreaterThanOrEqual(initial.length)
    expect(finalTree.some((row) => restartTree.some((old) => old.pid === row.pid))).toBe(false)
    await send(page, 'Recover after restarting the Session')
    await expect.poll(() => rootText(page)).toContain('Session recovered after restart cleanup.')
    await expect.poll(() => agentText(page, 'interrupt-child'))
      .not.toContain('INTERRUPT CHILD LATE OUTPUT')
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 12, remaining: 0, unmatchedRequests: 0, inFlight: 0, verified: true,
    })
    const requests = await desktop.llm!.requests()
    expect(requests.every((request) => request.matched)).toBe(true)
    expect(JSON.stringify(requests)).not.toContain('LATE OUTPUT')
  } finally {
    await closeDesktop(desktop)
  }
})

interface ProcessRow { readonly pid: number; readonly ppid: number; readonly command: string }

async function loopalTree(root: number): Promise<ProcessRow[]> {
  const { stdout } = await execFile('ps', ['-axo', 'pid=,ppid=,comm='])
  const rows = stdout.split('\n').flatMap((line): ProcessRow[] => {
    const match = line.match(/^\s*(\d+)\s+(\d+)\s+(.+)$/)
    return match ? [{ pid: Number(match[1]), ppid: Number(match[2]), command: match[3]!.trim() }] : []
  })
  const descendants = new Set([root])
  for (let changed = true; changed;) {
    changed = false
    for (const row of rows) if (descendants.has(row.ppid) && !descendants.has(row.pid)) {
      descendants.add(row.pid); changed = true
    }
  }
  return rows.filter((row) => descendants.has(row.pid) && basename(row.command) === 'loopal')
}

async function newChildProcess(
  root: number, before: readonly ProcessRow[], parentPid: number,
): Promise<ProcessRow> {
  const known = new Set(before.map((row) => row.pid))
  await expect.poll(async () => (await loopalTree(root)).filter((row) => !known.has(row.pid)).length)
    .toBe(1)
  const child = (await loopalTree(root)).find((row) => !known.has(row.pid))!
  expect(child.ppid).toBe(parentPid)
  return child
}

function isAlive(pid: number): boolean {
  try { process.kill(pid, 0); return true } catch { return false }
}

function livePids(rows: readonly ProcessRow[]): number[] {
  return rows.filter((row) => isAlive(row.pid)).map((row) => row.pid)
}

async function processParent(pid: number): Promise<number | undefined> {
  try {
    const { stdout } = await execFile('ps', ['-o', 'ppid=', '-p', String(pid)])
    const parent = Number(stdout.trim())
    return Number.isSafeInteger(parent) && parent > 0 ? parent : undefined
  } catch { return undefined }
}

async function expectAgent(page: Page, agentId: string, marker: string): Promise<void> {
  await expect.poll(async () => {
    const agent = await agentSnapshot(page, agentId)
    return { status: agent?.status, text: agent?.conversation?.map((entry) => entry.text).join('\n') }
  }).toEqual({ status: 'running', text: expect.stringContaining(marker) })
}

async function agentSnapshot(page: Page, agentId: string) {
  return page.evaluate(async (id) => {
    const bootstrap = await window.loopalDesktop.bootstrap()
    return (await window.loopalDesktop.openSession(bootstrap.activeSessionId!)).agents
      .find((agent) => agent.id === id)
  }, agentId)
}

async function agentStatus(page: Page, agentId: string): Promise<string | undefined> {
  return (await agentSnapshot(page, agentId))?.status
}

async function agentText(page: Page, agentId: string): Promise<string> {
  const agent = await agentSnapshot(page, agentId)
  return agent?.conversation?.map((entry) => entry.text).join('\n') ?? ''
}

async function rootText(page: Page): Promise<string> {
  return agentText(page, 'main')
}

async function bypass(page: Page, target: Awaited<ReturnType<typeof runtimeTarget>>): Promise<void> {
  await page.evaluate(async (value) => window.loopalDesktop.controlAgent({
    target: value, command: { type: 'permission', mode: 'bypass' },
  }), target)
}
