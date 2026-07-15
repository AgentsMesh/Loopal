import {
  Children, isValidElement, type ComponentPropsWithoutRef,
} from 'react'
import ReactMarkdown, { type Components, type UrlTransform } from 'react-markdown'
import remarkGfm from 'remark-gfm'

interface RichTextProps {
  readonly text: string
}

export function RichText({ text }: RichTextProps): React.JSX.Element {
  return (
    <div className="rich-text">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        components={components}
        allowedElements={allowedElements}
        unwrapDisallowed
        skipHtml
        urlTransform={safeUrl}
      >
        {text}
      </ReactMarkdown>
    </div>
  )
}

const components: Components = {
  a: ({ href, children }) => href
    ? <a href={href} target="_blank" rel="noreferrer">{children}</a>
    : <>{children}</>,
  img: ({ src, alt }) => src
    ? <a className="rich-image-link" href={src} target="_blank" rel="noreferrer">{alt || src}</a>
    : <span className="rich-image-link">{alt}</span>,
  pre: CodeBlock,
}

const allowedElements = [
  'a', 'blockquote', 'br', 'code', 'del', 'em', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
  'hr', 'img', 'input', 'li', 'ol', 'p', 'pre', 'strong', 'table', 'tbody', 'td',
  'th', 'thead', 'tr', 'ul',
]

function CodeBlock({ children, node: _node, ...props }: ComponentPropsWithoutRef<'pre'> & {
  readonly node?: unknown
}): React.JSX.Element {
  const child = Children.toArray(children)[0]
  const className = isValidElement<{ className?: string }>(child)
    ? child.props.className
    : undefined
  const language = className?.match(/(?:^|\s)language-([^\s]+)/)?.[1] ?? ''
  return <pre {...props} data-language={language}>{children}</pre>
}

const safeUrl: UrlTransform = (value): string => {
  try { return new URL(value).protocol === 'https:' ? value : '' }
  catch { return '' }
}
