import { useLayoutEffect, useRef } from 'react'
import { type ConversationEntry, type SessionView } from '../../../../shared/contracts'
import { RichText } from './rich-text'
import { ToolInvocationView } from './tool-invocation-view'
import { useI18n } from '../../../browser/i18n-context'
import { localizeRuntimeEventNotice } from './runtime-event-notice'

interface ConversationViewProps {
  readonly entries: readonly ConversationEntry[]
  readonly view?: SessionView
}

export function ConversationView(props: ConversationViewProps): React.JSX.Element {
  const { t } = useI18n()
  const feed = useRef<HTMLDivElement>(null)
  const followTail = useRef(true)
  useLayoutEffect(() => {
    const container = feed.current?.parentElement
    if (!container) return
    const trackPosition = (): void => {
      followTail.current = container.scrollHeight - container.scrollTop - container.clientHeight < 96
    }
    container.addEventListener('scroll', trackPosition, { passive: true })
    const resize = typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(() => {
      if (followTail.current) container.scrollTop = container.scrollHeight
    })
    resize?.observe(container)
    return () => {
      container.removeEventListener('scroll', trackPosition)
      resize?.disconnect()
    }
  }, [])
  useLayoutEffect(() => {
    const container = feed.current?.parentElement
    if (container && followTail.current) container.scrollTop = container.scrollHeight
  }, [props.entries, props.view])
  return (
    <div className="conversation-feed" ref={feed}>
      {props.view?.historyTruncated && (
        <div className="conversation-banner">{t('conversation.historyTruncated')}</div>
      )}
      {props.view?.retryBanner && <div className="conversation-banner warning">{props.view.retryBanner}</div>}
      {props.view?.compactBanner && <div className="conversation-banner">{props.view.compactBanner}</div>}
      {props.entries.map((entry) => <ConversationMessage key={entry.id} entry={entry} />)}
    </div>
  )
}

function ConversationMessage({ entry }: { readonly entry: ConversationEntry }): React.JSX.Element {
  const { t } = useI18n()
  const tools = entry.toolCalls ?? []
  const text = typeof entry.eventNotice === 'object'
    ? localizeRuntimeEventNotice(entry.eventNotice, t)
    : entry.role === 'user' && entry.skill ? entry.skill.userArgs : entry.text
  const label = roleLabel(entry.role, t)
  const showRole = ['thinking', 'error', 'welcome'].includes(entry.role)
  const showHeader = showRole || Boolean(
    (entry.agentId && entry.agentId !== 'main') || entry.skill || entry.inbox
      || entry.thinkingTokens !== undefined,
  )
  return (
    <article
      className={`message message-${entry.role} ${entry.streaming ? 'streaming' : ''} ${
        entry.eventNotice ? 'event-notice' : ''
      }`}
      data-message-role={entry.role}
      aria-label={label}
    >
      {showHeader && <header className="message-header">
        {showRole && <span className="message-role">{label}</span>}
        {entry.agentId && entry.agentId !== 'main' && <span>{entry.agentId}</span>}
        {entry.skill && <span>{t('conversation.skill', { name: entry.skill.name })}</span>}
        {entry.inbox && <span>{t('conversation.from', { source: entry.inbox.source })}</span>}
        {entry.thinkingTokens !== undefined && (
          <span>{t('conversation.tokens', { count: entry.thinkingTokens })}</span>
        )}
      </header>}
      {text && <RichText text={text} />}
      {entry.imageCount !== undefined && entry.imageCount > 0 && (
        <div className="message-attachments">{t('conversation.images', {
          count: entry.imageCount,
        })}</div>
      )}
      {tools.length > 0 && (
        <div className="tool-list">{tools.map((tool) => (
          <ToolInvocationView key={tool.id} tool={tool} />
        ))}</div>
      )}
      {entry.streaming && <span className="streaming-cursor" aria-label={t('conversation.streaming')} />}
    </article>
  )
}

function roleLabel(
  role: ConversationEntry['role'],
  t: ReturnType<typeof useI18n>['t'],
): string {
  if (role === 'assistant') return 'Loopal'
  if (role === 'thinking') return t('conversation.role.thinking')
  if (role === 'error') return t('conversation.role.error')
  if (role === 'welcome') return 'Loopal'
  return role === 'user' ? t('conversation.role.user') : role[0]!.toUpperCase() + role.slice(1)
}
