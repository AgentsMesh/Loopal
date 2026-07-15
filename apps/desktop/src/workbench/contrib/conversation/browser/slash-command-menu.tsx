import { type SlashCommandItem } from './slash-command-model'

interface SlashCommandMenuProps {
  readonly id: string
  readonly label: string
  readonly items: readonly SlashCommandItem[]
  readonly activeIndex: number
  readonly emptyLabel: string
  readonly onSelect: (item: SlashCommandItem) => void
  readonly onHover: (index: number) => void
}

export function SlashCommandMenu(props: SlashCommandMenuProps): React.JSX.Element {
  return (
    <div className="command-menu" id={props.id} role="listbox"
      aria-label={props.label} data-testid="command-menu">
      {props.items.length === 0 ? (
        <div className="command-menu-empty">{props.emptyLabel}</div>
      ) : props.items.map((item, index) => (
        <button
          type="button"
          role="option"
          id={`${props.id}-option-${index}`}
          aria-selected={index === props.activeIndex}
          className={index === props.activeIndex ? 'command-option selected' : 'command-option'}
          data-command-name={item.name}
          key={`${item.source}:${item.name}`}
          onMouseEnter={() => props.onHover(index)}
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => props.onSelect(item)}
        >
          <span className="command-option-copy">
            <strong>{item.name}</strong>
            <small>{item.description}</small>
          </span>
          <span className="command-option-meta">
            <span>{item.sourceLabel}</span>
            <code>{item.usage}</code>
          </span>
        </button>
      ))}
    </div>
  )
}
