import { useEffect, useId, useMemo, useState, type KeyboardEvent } from 'react'
import { type SlashCommandItem } from './slash-command-model'

interface CommandMenuOptions {
  readonly draft: string
  readonly items: readonly SlashCommandItem[]
  readonly helpQuery: string | undefined
  readonly onDraftChange: (value: string) => void
  readonly onExecuteCommand: ((command: string) => void) | undefined
  readonly onRequestCommands: (() => void) | undefined
  readonly onDismissHelp: (() => void) | undefined
}

export function useComposerCommandMenu(options: CommandMenuOptions) {
  const menuId = `command-menu-${useId()}`
  const draftQuery = slashQuery(options.draft)
  const helpMode = options.helpQuery !== undefined
  const query = helpMode ? options.helpQuery : draftQuery
  const token = query === undefined
    ? undefined : `${helpMode ? 'help' : 'draft'}:${options.draft}:${query}`
  const [dismissedToken, setDismissedToken] = useState<string>()
  const [activeIndex, setActiveIndex] = useState(0)
  const items = useMemo(
    () => filterCommands(options.items, query).slice(0, 12),
    [options.items, query],
  )
  const visible = token !== undefined && token !== dismissedToken

  useEffect(() => {
    setActiveIndex(0)
    if (token !== undefined) options.onRequestCommands?.()
  }, [token, options.onRequestCommands])
  useEffect(() => {
    if (activeIndex >= items.length) setActiveIndex(Math.max(0, items.length - 1))
  }, [activeIndex, items.length])

  const dismiss = (): void => {
    setDismissedToken(token)
    if (helpMode) options.onDismissHelp?.()
  }
  const fill = (item: SlashCommandItem): void => {
    const next = item.arguments === 'none' ? item.name : `${item.name} `
    setDismissedToken(`draft:${next}:${slashQuery(next) ?? ''}`)
    options.onDismissHelp?.()
    options.onDraftChange(next)
  }
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>): boolean => {
    if (!visible) return false
    if (event.key === 'Escape') {
      event.preventDefault()
      dismiss()
      return true
    }
    if (!items.length) return false
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const change = event.key === 'ArrowDown' ? 1 : -1
      setActiveIndex((index) => (index + change + items.length) % items.length)
      return true
    }
    const selected = items[activeIndex]
    if (!selected || (event.key !== 'Tab' && event.key !== 'Enter')) return false
    event.preventDefault()
    if (event.key === 'Enter' && !helpMode && selected.source === 'runtime'
      && selected.arguments === 'none' && options.onExecuteCommand) {
      setDismissedToken(token)
      options.onExecuteCommand(selected.name)
    } else fill(selected)
    return true
  }

  return {
    menuId, items, visible, activeIndex, setActiveIndex,
    activeDescendant: visible && items.length ? `${menuId}-option-${activeIndex}` : undefined,
    onKeyDown, select: fill,
  }
}

function slashQuery(draft: string): string | undefined {
  const value = draft.trimStart()
  if (!value.startsWith('/') || value.startsWith('//') || /\s/.test(value)) return undefined
  return value.slice(1).toLocaleLowerCase()
}

function filterCommands(
  items: readonly SlashCommandItem[], query: string | undefined,
): readonly SlashCommandItem[] {
  if (query === undefined) return []
  const normalized = query.trim().replace(/^\//, '').toLocaleLowerCase()
  if (!normalized) return items
  const prefix = items.filter(
    (item) => item.name.slice(1).toLocaleLowerCase().startsWith(normalized),
  )
  if (prefix.length) return prefix
  return items.filter((item) => item.description.toLocaleLowerCase().includes(normalized))
}
