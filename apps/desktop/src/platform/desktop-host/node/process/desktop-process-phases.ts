import { DeferredPromise } from '../../../../base/common/async'
import {
  type DesktopActivationHandshake,
  type DesktopAliveHandshake,
  type DesktopHandshake,
  type DesktopReadyHandshake,
} from '../../common/desktop-handshake'

export class DesktopProcessPhases {
  private readonly alivePhase = new DeferredPromise<DesktopAliveHandshake>()
  private readonly createdPhase = new DeferredPromise<DesktopActivationHandshake>()
  private readonly readyPhase = new DeferredPromise<DesktopReadyHandshake>()
  readonly alive = this.alivePhase.promise
  readonly sessionCreated = this.createdPhase.promise
  readonly ready = this.readyPhase.promise
  private aliveSeen = false
  private createdId?: string
  private readyId?: string
  private failed = false

  constructor() {
    void this.alive.catch(() => undefined)
    void this.sessionCreated.catch(() => undefined)
    void this.ready.catch(() => undefined)
  }

  accept(handshake: DesktopHandshake): boolean {
    if (this.failed) return false
    if (handshake.phase === 'alive') {
      if (this.aliveSeen) {
        this.reject(new Error('Duplicate Desktop alive handshake'))
        return false
      }
      this.aliveSeen = true
      this.alivePhase.resolve(handshake)
      return true
    }
    if (handshake.phase === 'error') {
      this.reject(new Error(`${handshake.code}: ${handshake.message}`))
      return false
    }
    if (!this.aliveSeen) {
      this.reject(new Error(`Desktop ${handshake.phase} handshake preceded alive`))
      return false
    }
    if (handshake.phase === 'session_created') {
      if (this.readyId) {
        this.reject(new Error('Desktop created Session after ready'))
        return false
      }
      if (this.createdId && this.createdId !== handshake.session_id) {
        this.reject(new Error('Desktop Host changed Session during creation'))
        return false
      }
      this.createdId = handshake.session_id
      this.createdPhase.resolve(handshake)
      return true
    }
    if (handshake.phase === 'ready') {
      if (this.readyId && this.readyId !== handshake.session_id) {
        this.reject(new Error('Desktop Host changed Session after ready'))
        return false
      }
      if (this.createdId && this.createdId !== handshake.session_id) {
        this.reject(new Error('Desktop Host changed Session during startup'))
        return false
      }
      this.readyId = handshake.session_id
      this.createdPhase.resolve(handshake)
      this.readyPhase.resolve(handshake)
      return true
    }
    return false
  }

  reject(error: unknown): void {
    this.failed = true
    this.alivePhase.reject(error)
    this.createdPhase.reject(error)
    this.readyPhase.reject(error)
  }
}
