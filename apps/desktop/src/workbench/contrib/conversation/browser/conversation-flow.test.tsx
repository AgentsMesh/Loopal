import { render, screen } from '@testing-library/react'
import { type ConversationEntry } from '../../../../shared/contracts'
import { ConversationView } from './conversation-view'

const createdAt = '2026-07-14T12:00:00.000Z'

describe('Conversation transcript', () => {
  it('presents prompts and responses as a readable turn instead of a role log', () => {
    const entries: ConversationEntry[] = [
      entry('user', 'Please **review** this'),
      entry('thinking', 'Checking the implementation'),
      entry('assistant', '## Result\n\n- Correct\n- Tested'),
      entry('system', 'Context compacted.'),
    ]
    const { container } = render(<ConversationView entries={entries} />)
    const messages = Array.from(container.querySelectorAll<HTMLElement>('[data-message-role]'))

    expect(messages.map((message) => message.dataset.messageRole)).toEqual([
      'user', 'thinking', 'assistant', 'system',
    ])
    expect(screen.getByRole('article', { name: 'User' })).toHaveClass('message-user')
    expect(screen.getByRole('article', { name: 'User' }).querySelector('header')).toBeNull()
    expect(screen.getByRole('article', { name: 'Loopal' }).querySelector('header')).toBeNull()
    expect(screen.getByRole('article', { name: 'Thinking' })).toHaveTextContent(
      'Checking the implementation',
    )
    expect(screen.getByRole('article', { name: 'System' }).querySelector('header')).toBeNull()
    expect(screen.getByRole('heading', { name: 'Result' })).toBeVisible()
    expect(screen.getByRole('list')).toHaveTextContent('Correct')
  })
})

function entry(role: ConversationEntry['role'], text: string): ConversationEntry {
  return { id: `${role}-${text}`, role, text, createdAt }
}
