import { useState } from 'react'
import { type Artifact } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

export function ArtifactPanel(props: {
  readonly artifacts: readonly Artifact[]
}): React.JSX.Element {
  const { t } = useI18n()
  const [expandedId, setExpandedId] = useState<string>()
  return (
    <div className="inspector-content" data-testid="artifacts-pane">
      {props.artifacts.map((artifact) => (
        <article className="artifact-item" key={artifact.id}>
          <button
            className="artifact-card"
            aria-expanded={expandedId === artifact.id}
            onClick={() => setExpandedId((current) => (
              current === artifact.id ? undefined : artifact.id
            ))}
          >
            <span className="artifact-icon">◇</span>
            <span><strong>{artifact.title}</strong><small>{artifact.mediaType}</small></span>
          </button>
          {expandedId === artifact.id && (
            <dl className="artifact-metadata">
              <div><dt>{t('artifact.kind')}</dt><dd>{artifact.kind}</dd></div>
              <div><dt>{t('artifact.producer')}</dt><dd>{artifact.producerAgentId}</dd></div>
              <div><dt>{t('artifact.uri')}</dt><dd>{artifact.uri}</dd></div>
            </dl>
          )}
        </article>
      ))}
      {props.artifacts.length === 0 && (
        <div className="empty-inspector">
          <span>◫</span><p>{t('artifact.empty')}</p>
        </div>
      )}
    </div>
  )
}
