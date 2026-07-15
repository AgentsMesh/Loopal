import { SessionRuntimeEntry } from './session-runtime-registry-entry'
import { type SessionRuntimeActivation } from './session-runtime-registry-types'

interface HostSession {
  readonly sessionId: string
}

export class SessionRuntimeActivationGate {
  private sessionId?: string
  private activation?: Promise<void>

  constructor(
    private readonly entry: SessionRuntimeEntry,
    private readonly callback?: SessionRuntimeActivation,
  ) {}

  readonly activate = (session: HostSession): Promise<void> => {
    if (this.sessionId && this.sessionId !== session.sessionId) {
      return Promise.reject(new Error('Desktop Host changed Session during activation'))
    }
    if (!this.activation) {
      this.sessionId = session.sessionId
      this.entry.markSessionCreated(session.sessionId)
      this.activation = Promise.resolve().then(async () => {
        await this.callback?.(session.sessionId)
      })
    }
    return this.activation
  }
}
