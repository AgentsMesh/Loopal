import { act, renderHook } from '@testing-library/react'
import { createTestAPI } from '../../../../../test/support/workbench/api-stub'
import { useSlashCommands } from './use-slash-commands'

const revision = 'a'.repeat(64)
const skill = (
  name: string, effective = true, hasArguments = false,
) => ({
  name, effective, hasArguments, revision, editable: true,
  description: `${name} description`, source: 'fixture', scope: 'global' as const,
})

describe('useSlashCommands', () => {
  it('merges effective Skills while built-ins win name collisions', async () => {
    const listSkills = vi.fn(async (workspaceId: string) => ({
      workspaceId,
      skills: [skill('/desktop-check', true, true), skill('/plan'), skill('/hidden', false)],
    }))
    const { api } = createTestAPI({ listSkills })
    const hook = renderHook(() => useSlashCommands(api, 'workspace-1'))
    await act(async () => hook.result.current.refresh())
    expect(listSkills).toHaveBeenCalledWith('workspace-1')
    expect(hook.result.current.items.filter(({ name }) => name === '/plan')).toHaveLength(1)
    expect(hook.result.current.items.find(({ name }) => name === '/plan')?.source).toBe('runtime')
    expect(hook.result.current.items.find(({ name }) => name === '/desktop-check'))
      .toMatchObject({ source: 'skill', arguments: 'optional' })
    expect(hook.result.current.items.some(({ name }) => name === '/hidden')).toBe(false)
  })

  it('routes controls locally while Skills and unknown slash input remain messages', async () => {
    const { api } = createTestAPI()
    const hook = renderHook(() => useSlashCommands(api, 'workspace-1'))
    const control = vi.fn(async () => true)
    await expect(hook.result.current.execute('/plan', 'main', control)).resolves.toBe('handled')
    expect(control).toHaveBeenCalledWith('main', { type: 'mode', mode: 'plan' })
    await expect(hook.result.current.execute('/desktop-check alpha', 'main', control))
      .resolves.toBe('message')
    await expect(hook.result.current.execute('/unknown', 'main', control))
      .resolves.toBe('message')
  })

  it('blocks invalid parameters and failed controls without losing command context', async () => {
    const { api } = createTestAPI()
    const hook = renderHook(() => useSlashCommands(api, 'workspace-1'))
    const control = vi.fn(async () => true)
    await act(async () => {
      expect(await hook.result.current.execute(
        '/permission unrestricted', 'main', control,
      )).toBe('blocked')
    })
    expect(control).not.toHaveBeenCalled()
    expect(hook.result.current.error).toContain('/permission')
    await act(async () => {
      expect(await hook.result.current.execute('/plan', 'main', control, true)).toBe('blocked')
    })
    expect(hook.result.current.error).toContain('images')
    expect(control).not.toHaveBeenCalled()
    const rejected = vi.fn(async () => false)
    await expect(hook.result.current.execute('/clear', 'main', rejected)).resolves.toBe('blocked')
  })

  it('opens help as a local catalog action', async () => {
    const { api } = createTestAPI()
    const hook = renderHook(() => useSlashCommands(api, 'workspace-1'))
    await act(async () => {
      expect(await hook.result.current.execute('/help plan', 'main', vi.fn()))
        .toBe('handled')
    })
    expect(hook.result.current.helpQuery).toBe('plan')
  })
})
