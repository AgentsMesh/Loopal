import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { vi } from 'vitest'
import {
  LoopalMetaHubCoordinator,
  type SpawnMetaHubProcess,
} from './loopal-metahub-coordinator'

class FakeChild extends EventEmitter {
  readonly pid: number
  readonly stdout = new PassThrough()
  readonly stderr = new PassThrough()
  exitCode: number | null = null
  readonly kill = vi.fn((signal: NodeJS.Signals) => {
    queueMicrotask(() => this.exit(null, signal))
    return true
  })
  constructor(pid: number) { super(); this.pid = pid }
  exit(code: number | null, signal: NodeJS.Signals | null): void {
    if (this.exitCode !== null) return
    this.exitCode = code ?? (signal ? 1 : 0)
    this.emit('exit', code, signal)
  }
}

function handshake(child: FakeChild, token = 'private-token'): void {
  child.stdout.write(`LOOPAL_METAHUB ${JSON.stringify({
    protocol_version: 1,
    phase: 'ready',
    address: '127.0.0.1:4567',
    token,
    pid: child.pid,
    parent_pid: 321,
  })}\n`)
}

describe('LoopalMetaHubCoordinator', () => {
  it('uses shell-free machine startup and never exposes the token in public status', async () => {
    const child = new FakeChild(654)
    const spawn = vi.fn(() => child as never) as unknown as SpawnMetaHubProcess
    const coordinator = new LoopalMetaHubCoordinator('/bin/loopal', 321, spawn)
    const pending = coordinator.start('127.0.0.1:0')
    handshake(child)
    await expect(pending).resolves.toEqual({
      address: '127.0.0.1:4567', token: 'private-token',
    })
    expect(spawn).toHaveBeenCalledWith('/bin/loopal', [
      '--meta-hub', '127.0.0.1:0', '--meta-hub-parent-pid', '321',
    ])
    expect(coordinator.status).toEqual({ state: 'running', address: '127.0.0.1:4567' })
    expect(coordinator.ownedAddress).toBe('127.0.0.1:4567')
    expect(JSON.stringify(coordinator.status)).not.toContain('private-token')
    await coordinator.stop()
    expect(child.kill).toHaveBeenCalledWith('SIGTERM')
    expect(coordinator.status).toEqual({ state: 'stopped' })
    expect(coordinator.ownedAddress).toBeUndefined()
  })

  it('keeps stop authoritative when startup is interrupted', async () => {
    const child = new FakeChild(700)
    const coordinator = new LoopalMetaHubCoordinator(
      '/bin/loopal', 321, () => child as never,
    )
    const pending = coordinator.start('127.0.0.1:0')
    await coordinator.stop()
    await expect(pending).rejects.toThrow()
    expect(coordinator.status).toEqual({ state: 'stopped' })
  })

  it('supports rapid stop and restart without stale generation state', async () => {
    const first = new FakeChild(701)
    const second = new FakeChild(702)
    const spawn = vi.fn()
      .mockReturnValueOnce(first as never)
      .mockReturnValueOnce(second as never)
    const coordinator = new LoopalMetaHubCoordinator('/bin/loopal', 321, spawn)
    const firstStart = coordinator.start('127.0.0.1:0')
    const stop = coordinator.stop()
    const secondStart = coordinator.start('127.0.0.1:0')
    await Promise.allSettled([firstStart, stop])
    await vi.waitFor(() => expect(spawn).toHaveBeenCalledTimes(2))
    handshake(second, 'second-secret')
    await expect(secondStart).resolves.toEqual({
      address: '127.0.0.1:4567', token: 'second-secret',
    })
    expect(coordinator.status.state).toBe('running')
    await coordinator.stop()
  })

  it('retains the owned address after a crash until explicit stop', async () => {
    const child = new FakeChild(703)
    const coordinator = new LoopalMetaHubCoordinator('/bin/loopal', 321, () => child as never)
    const pending = coordinator.start('127.0.0.1:0')
    handshake(child)
    await pending
    child.exit(9, null)
    expect(coordinator.status.state).toBe('failed')
    expect(coordinator.ownedAddress).toBe('127.0.0.1:4567')
    await coordinator.stop()
    expect(coordinator.ownedAddress).toBeUndefined()
  })
})
