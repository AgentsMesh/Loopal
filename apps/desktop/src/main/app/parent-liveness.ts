import { type IDisposable, toDisposable } from '../../base/common/lifecycle'

export function monitorParent(
  parentPid: number,
  onMissing: () => void,
  probe: (pid: number) => void = defaultProbe,
  interval = 1_000,
): IDisposable {
  let missing = false
  const timer = setInterval(() => {
    if (missing) return
    try {
      probe(parentPid)
    } catch {
      missing = true
      onMissing()
    }
  }, interval)
  timer.unref()
  return toDisposable(() => clearInterval(timer))
}

function defaultProbe(pid: number): void {
  process.kill(pid, 0)
}
