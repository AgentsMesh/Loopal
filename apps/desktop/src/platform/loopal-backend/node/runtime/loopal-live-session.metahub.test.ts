import { expect, it, vi } from 'vitest'
import {
  liveSessionHarness as harness,
} from './loopal-live-session.test-fixtures'

it('polls cluster topology off the snapshot path and adds then removes remote Agents', async () => {
  vi.useFakeTimers()
  const value = harness()
  const base = value.request.getMockImplementation()!
  let remote = true
  value.request.mockImplementation(async (method, params, signal) => {
    if (method === 'hub/status') return remote ? {
      agent_count: 1,
      uplink: { connected: true, hub_name: 'hub-a', address: 'meta:9' },
    } : { agent_count: 1, uplink: null }
    if (method === 'meta/list_hubs') return { hubs: [
      { name: 'hub-a', status: 'Connected', agent_count: 1, capabilities: [] },
      { name: 'hub-b', status: 'Connected', agent_count: 1, capabilities: [] },
    ] }
    if (method === 'meta/topology') return { hubs: [
      { hub: 'hub-a', topology: { agents: [] } },
      { hub: 'hub-b', topology: { agents: [{
        name: 'main', parent: null, children: [], lifecycle: 'running',
      }] } },
    ] }
    return base(method, params, signal)
  })
  await value.state.initialize()
  await vi.advanceTimersByTimeAsync(1)
  expect(value.state.detail.agents).toContainEqual(
    expect.objectContaining({ id: 'hub-b/main', qualifiedName: 'hub-b/main' }),
  )
  remote = false
  await vi.advanceTimersByTimeAsync(2_001)
  expect(value.state.detail.agents.map((agent) => agent.id)).not.toContain('hub-b/main')
  expect(value.events.filter((event) => event.type === 'session_detail_replaced').length)
    .toBeGreaterThanOrEqual(2)
  value.state.dispose()
  vi.useRealTimers()
})
