import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  type LocalMetaHubStatus,
  type LoopalDesktopAPI,
  type MetaHubRuntimeState,
  type MetaHubSettings,
  type RuntimeSummary,
  type SessionSummary,
} from '../../../../shared/contracts'
import { federationHubName } from '../../../../shared/contracts/metahub-identity'
import {
  aggregateFederation, federationTargets, type FederationConnection,
} from './federation-model'

const stopped: LocalMetaHubStatus = { state: 'stopped' }

export function useFederationController(
  api: LoopalDesktopAPI,
  sessions: readonly SessionSummary[],
  runtimes: readonly RuntimeSummary[],
) {
  const targets = useMemo(() => federationTargets(sessions, runtimes), [runtimes, sessions])
  const targetsRef = useRef(targets)
  const request = useRef(0)
  const [local, setLocal] = useState<LocalMetaHubStatus>(stopped)
  const [settings, setSettings] = useState<MetaHubSettings>()
  const [connections, setConnections] = useState<readonly FederationConnection[]>([])
  const [busy, setBusy] = useState<string>()
  const [error, setError] = useState<string>()
  targetsRef.current = targets

  const refresh = useCallback(async (reportError = true): Promise<void> => {
    const version = ++request.current
    const currentTargets = Object.values(targetsRef.current)
    const [settingsResult, localResult, states] = await Promise.all([
      api.getMetaHubSettings().then(ok, failed),
      api.getLocalMetaHubStatus().then(ok, failed),
      Promise.all(currentTargets.map(async (target) => ({
        target,
        result: await api.getMetaHubStatus(target).then(ok, failed),
      }))),
    ])
    if (version !== request.current) return
    if (settingsResult.ok) setSettings(settingsResult.value)
    if (localResult.ok) setLocal(localResult.value)
    const next = states.map(({ target, result }): FederationConnection => ({
      target,
      state: result.ok ? result.value : {
        state: 'error', hubs: [], topology: [], error: message(result.error),
        refreshedAt: new Date().toISOString(),
      },
    }))
    setConnections(next)
    const failure = [settingsResult, localResult, ...states.map(({ result }) => result)]
      .find((result) => !result.ok)
    if (reportError && failure && !failure.ok) setError(message(failure.error))
    else if (reportError) setError(undefined)
  }, [api])

  useEffect(() => { void refresh() }, [refresh, targets])
  useEffect(() => {
    if (local.state !== 'running' && !connections.some(({ state }) => state.state === 'connected')) {
      return undefined
    }
    const timer = window.setInterval(() => void refresh(false), 2_000)
    return () => window.clearInterval(timer)
  }, [connections, local.state, refresh])

  const run = useCallback(async (key: string, operation: () => Promise<void>) => {
    if (busy) return
    setBusy(key); setError(undefined)
    try {
      let operationError: unknown
      try { await operation() }
      catch (reason) { operationError = reason }
      try { await refresh(operationError === undefined) }
      catch (reason) { if (operationError === undefined) setError(message(reason)) }
      if (operationError !== undefined) setError(message(operationError))
    } finally { setBusy(undefined) }
  }, [busy, refresh])

  const start = (): Promise<void> => run('start', async () => {
    await api.startLocalMetaHub({ bindAddress: '127.0.0.1:0' })
    const current = await api.getMetaHubSettings()
    await api.updateMetaHubSettings({
      address: current.address,
      hubName: current.hubName,
      joinOnStart: false,
      startLocalOnLaunch: true,
    })
  })
  const join = (sessionId: string): Promise<void> => run(`session:${sessionId}`, async () => {
    const target = targetsRef.current[sessionId]
    if (!target) throw new Error(`Session runtime is unavailable: ${sessionId}`)
    const current = await api.getMetaHubSettings()
    await api.joinMetaHub({ ...target, hubName: federationHubName(current.hubName, target) })
  })
  const leave = (sessionId: string): Promise<void> => run(`session:${sessionId}`, async () => {
    const target = targetsRef.current[sessionId]
    if (!target) throw new Error(`Session runtime is unavailable: ${sessionId}`)
    await api.disconnectMetaHub(target)
  })

  const address = settings?.address.trim()
    || (local.state === 'running' ? local.address?.trim() : undefined)
  const snapshot = aggregateFederation(local, targets, connections, address)
  const memberships = Object.fromEntries(Object.entries(snapshot.memberships).map(
    ([sessionId, state]) => [sessionId,
      state === 'disconnected' && (!settings?.address || !settings.tokenConfigured)
        ? 'unavailable' as const : state],
  ))
  return {
    snapshot: { ...snapshot, memberships }, settings,
    busy, error, refresh: () => refresh(), start, join, leave,
  }
}

type Result<T> = { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly error: unknown }
function ok<T>(value: T): Result<T> { return { ok: true, value } }
function failed(error: unknown): Result<never> { return { ok: false, error } }
function message(value: unknown): string { return value instanceof Error ? value.message : String(value) }
