import { describe, expect, it, vi } from 'vitest'
import { toDisposable } from '../../../base/common/lifecycle'
import { ContributionRegistry } from './contribution-registry'

describe('ContributionRegistry', () => {
  it('activates once and disposes contribution resources', () => {
    const registry = new ContributionRegistry()
    const cleanup = vi.fn()
    const activate = vi.fn(({ disposables }) => disposables.add(toDisposable(cleanup)))
    const registration = registry.register({ id: 'tasks', activate })
    registry.activateAll()
    registry.activateAll()
    expect(activate).toHaveBeenCalledOnce()
    registration.dispose()
    registration.dispose()
    expect(cleanup).toHaveBeenCalledOnce()
    registry.dispose()
  })

  it('rejects duplicate contributions and disposes all active entries', () => {
    const registry = new ContributionRegistry()
    const cleanup = vi.fn()
    const contribution = {
      id: 'agents',
      activate: ({ disposables }: { disposables: { add(value: { dispose(): void }): unknown } }) =>
        disposables.add(toDisposable(cleanup)),
    }
    registry.register(contribution)
    expect(() => registry.register(contribution)).toThrow('already registered')
    registry.activateAll()
    registry.dispose()
    expect(cleanup).toHaveBeenCalledOnce()
  })
})
