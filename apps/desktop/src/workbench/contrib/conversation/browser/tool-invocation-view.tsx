import { type ToolInvocation } from '../../../../shared/contracts'
import { type MessageKey } from '../../../../shared/i18n'
import { useI18n } from '../../../browser/i18n-context'

interface ToolInvocationViewProps {
  readonly tool: ToolInvocation
}

const labels: Record<ToolInvocation['status'], MessageKey> = {
  pending: 'tool.status.pending',
  running: 'tool.status.running',
  succeeded: 'tool.status.succeeded',
  failed: 'tool.status.failed',
  stale: 'tool.status.stale',
  cancelled: 'tool.status.cancelled',
}

export function ToolInvocationView({ tool }: ToolInvocationViewProps): React.JSX.Element {
  const { t } = useI18n()
  const active = tool.status === 'pending' || tool.status === 'running'
  return (
    <details
      className={`tool-invocation tool-${tool.status}`}
      open={active || tool.status === 'failed'}
      data-testid="tool-invocation"
    >
      <summary>
        <span className="tool-state" aria-label={t(labels[tool.status])} />
        <strong>{toolLabel(tool)}</strong>
        <small>{t(labels[tool.status])}{formatDuration(tool.durationMs)}</small>
      </summary>
      <div className="tool-body">
        {tool.progress && <pre className="tool-progress">{tool.progress}</pre>}
        {tool.detail && <p className="tool-detail">{tool.detail}</p>}
        {tool.output && <pre className="tool-output">{tool.output}</pre>}
        {tool.input !== undefined && (
          <details className="tool-input">
            <summary>{t('tool.input')}</summary>
            <pre>{formatValue(tool.input)}</pre>
          </details>
        )}
      </div>
    </details>
  )
}

function toolLabel(tool: ToolInvocation): string {
  const summary = tool.summary.trim()
  if (!summary || summary === tool.name) return tool.name
  const normalizedName = tool.name.replace(/[^a-z0-9]/gi, '').toLowerCase()
  const callName = summary.match(/^([^({\s]+)\s*[({]/)?.[1]
    ?.replace(/[^a-z0-9]/gi, '').toLowerCase()
  if (callName === normalizedName || /^(?:\{|\[)/.test(summary)) return tool.name
  const compact = summary.length > 96 ? `${summary.slice(0, 93).trimEnd()}…` : summary
  return `${tool.name} · ${compact}`
}

function formatValue(value: unknown): string {
  if (typeof value === 'string') return value
  try { return JSON.stringify(value, null, 2) }
  catch { return String(value) }
}

function formatDuration(value: number | undefined): string {
  if (value === undefined) return ''
  return value < 1_000 ? ` · ${Math.round(value)}ms` : ` · ${(value / 1_000).toFixed(1)}s`
}
