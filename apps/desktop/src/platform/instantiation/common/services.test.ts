import { describe, expect, it, vi } from 'vitest'
import { ServiceScope } from './scope'
import {
  CyclicServiceDependencyError,
  InstantiationService,
  MissingServiceError,
  ServiceCollection,
  createServiceIdentifier,
} from './services'

describe('instantiation services', () => {
  it('stores instances, factories, and service identifiers', () => {
    const IValue = createServiceIdentifier<number>('value')
    const collection = new ServiceCollection()
    expect(collection.has(IValue)).toBe(false)
    collection.setInstance(IValue, 4)
    expect(collection.has(IValue)).toBe(true)
    const service = new InstantiationService(collection)
    expect(service.get(IValue)).toBe(4)
    expect(IValue.toString()).toBe('value')
    expect(service.invokeFunction((accessor) => accessor.get(IValue) + 1)).toBe(5)
  })

  it('lazily creates and caches owned disposable services', () => {
    const IResource = createServiceIdentifier<{ dispose(): void; value: number }>('resource')
    const dispose = vi.fn()
    const factory = vi.fn(() => ({ dispose, value: 9 }))
    const collection = new ServiceCollection().setFactory(IResource, factory)
    const service = new InstantiationService(collection)
    expect(service.get(IResource).value).toBe(9)
    expect(service.get(IResource).value).toBe(9)
    expect(factory).toHaveBeenCalledOnce()
    service.dispose()
    service.dispose()
    expect(dispose).toHaveBeenCalledOnce()
    expect(() => service.get(IResource)).toThrow('disposed')
    expect(() => service.invokeFunction(() => 1)).toThrow('disposed')
    expect(() => service.createChild()).toThrow('disposed')
  })

  it('resolves parent services and child overrides', () => {
    const IValue = createServiceIdentifier<number>('value')
    const parent = new InstantiationService(new ServiceCollection().setInstance(IValue, 1))
    const child = parent.createChild()
    expect(child.get(IValue)).toBe(1)
    const override = parent.createChild(new ServiceCollection().setInstance(IValue, 2))
    expect(override.get(IValue)).toBe(2)
    child.dispose()
    override.dispose()
    parent.dispose()
  })

  it('reports missing and cyclic dependencies', () => {
    const IMissing = createServiceIdentifier<string>('missing')
    const service = new InstantiationService(new ServiceCollection())
    expect(() => service.get(IMissing)).toThrow(MissingServiceError)

    const ILeft = createServiceIdentifier<object>('left')
    const IRight = createServiceIdentifier<object>('right')
    const cyclicCollection = new ServiceCollection()
      .setFactory(ILeft, (accessor) => ({ right: accessor.get(IRight) }))
      .setFactory(IRight, (accessor) => ({ left: accessor.get(ILeft) }))
    const cyclic = new InstantiationService(cyclicCollection)
    expect(() => cyclic.get(ILeft)).toThrow(CyclicServiceDependencyError)
    expect(() => cyclic.get(ILeft)).toThrow('left -> right -> left')
  })

  it('defines the intended service scopes', () => {
    expect(Object.values(ServiceScope)).toEqual(['app', 'window', 'workspace', 'pane'])
  })
})
