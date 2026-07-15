import { DisposableStore, type IDisposable, isDisposable } from '../../../base/common/lifecycle'

export class ServiceIdentifier<T> {
  readonly _serviceBrand!: T

  constructor(readonly id: string) {}

  toString(): string {
    return this.id
  }
}

export function createServiceIdentifier<T>(id: string): ServiceIdentifier<T> {
  return new ServiceIdentifier<T>(id)
}

export interface ServicesAccessor {
  get<T>(identifier: ServiceIdentifier<T>): T
}

export type ServiceFactory<T> = (accessor: ServicesAccessor) => T

type ServiceEntry<T> =
  | { readonly kind: 'instance'; readonly value: T; readonly owned: boolean }
  | { readonly kind: 'factory'; readonly factory: ServiceFactory<T> }

export class MissingServiceError extends Error {
  constructor(identifier: ServiceIdentifier<unknown>) {
    super(`Service is not registered: ${identifier.id}`)
    this.name = 'MissingServiceError'
  }
}

export class CyclicServiceDependencyError extends Error {
  constructor(path: readonly string[]) {
    super(`Cyclic service dependency: ${path.join(' -> ')}`)
    this.name = 'CyclicServiceDependencyError'
  }
}

export class ServiceCollection {
  private readonly entries = new Map<ServiceIdentifier<unknown>, ServiceEntry<unknown>>()

  setInstance<T>(identifier: ServiceIdentifier<T>, instance: T, owned = false): this {
    this.entries.set(identifier, { kind: 'instance', value: instance, owned })
    return this
  }

  setFactory<T>(identifier: ServiceIdentifier<T>, factory: ServiceFactory<T>): this {
    this.entries.set(identifier, { kind: 'factory', factory })
    return this
  }

  has(identifier: ServiceIdentifier<unknown>): boolean {
    return this.entries.has(identifier)
  }

  get<T>(identifier: ServiceIdentifier<T>): ServiceEntry<T> | undefined {
    return this.entries.get(identifier) as ServiceEntry<T> | undefined
  }
}

export class InstantiationService implements ServicesAccessor, IDisposable {
  private readonly owned = new DisposableStore()
  private readonly resolving: ServiceIdentifier<unknown>[] = []
  private disposed = false

  constructor(
    private readonly services: ServiceCollection,
    private readonly parent?: InstantiationService,
  ) {}

  get<T>(identifier: ServiceIdentifier<T>): T {
    if (this.disposed) {
      throw new Error('InstantiationService is disposed')
    }
    const entry = this.services.get(identifier)
    if (!entry) {
      if (this.parent) {
        return this.parent.get(identifier)
      }
      throw new MissingServiceError(identifier)
    }
    if (entry.kind === 'instance') {
      return entry.value
    }
    return this.createFromFactory(identifier, entry.factory)
  }

  invokeFunction<T>(callback: (accessor: ServicesAccessor) => T): T {
    if (this.disposed) {
      throw new Error('InstantiationService is disposed')
    }
    return callback(this)
  }

  createChild(services = new ServiceCollection()): InstantiationService {
    if (this.disposed) {
      throw new Error('InstantiationService is disposed')
    }
    return new InstantiationService(services, this)
  }

  dispose(): void {
    if (this.disposed) {
      return
    }
    this.disposed = true
    this.owned.dispose()
  }

  private createFromFactory<T>(
    identifier: ServiceIdentifier<T>,
    factory: ServiceFactory<T>,
  ): T {
    const cycleIndex = this.resolving.indexOf(identifier)
    if (cycleIndex >= 0) {
      const path = this.resolving.slice(cycleIndex).map((item) => item.id)
      path.push(identifier.id)
      throw new CyclicServiceDependencyError(path)
    }
    this.resolving.push(identifier)
    try {
      const instance = factory(this)
      this.services.setInstance(identifier, instance, true)
      if (isDisposable(instance)) {
        this.owned.add(instance)
      }
      return instance
    } finally {
      this.resolving.pop()
    }
  }
}
