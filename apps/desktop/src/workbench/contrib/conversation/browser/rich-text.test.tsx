import { render, screen } from '@testing-library/react'
import { RichText } from './rich-text'

describe('RichText', () => {
  it('renders bounded Markdown blocks and safe inline content', () => {
    const text = [
      '# Heading one', '## Heading two', '### Heading three',
      '#### Heading four', '##### Heading five', '###### Heading six', '',
      '> A **quoted** line', '> and another line', '',
      '1. Ordered `one`', '2. Ordered two',
      '- [Safe](https://loopal.ai)',
      '* [Unsafe](http://loopal.ai)',
      '+ [Broken](not a url)', '',
      '**Bold only**',
      'Plain **bold**, *italic*, ~~removed~~ and `code` with [Docs](https://loopal.ai/docs).', '',
      '| State | Result |', '| --- | --- |', '| ready | yes |', '',
      '- [x] verified', '- [ ] pending', '', '---', '',
      '```ts', 'const ready = true', '```',
      '```', 'unterminated fence',
      '<script>window.compromised = true</script>',
    ].join('\r\n')
    const { container } = render(<RichText text={text} />)

    for (let level = 1; level <= 6; level++) {
      expect(container.querySelector(`h${level}`)).toHaveTextContent(`Heading ${word(level)}`)
    }
    expect(container.querySelector('blockquote')).toHaveTextContent('A quoted line')
    expect(container.querySelector('ol')).toHaveTextContent('Ordered one')
    expect(screen.getByText('Unsafe').closest('ul')).toHaveTextContent('Unsafe')
    expect(screen.getByRole('link', { name: 'Safe' })).toHaveAttribute(
      'href', 'https://loopal.ai',
    )
    expect(screen.getByRole('link', { name: 'Docs' })).toHaveAttribute('target', '_blank')
    expect(screen.getByText('Unsafe').closest('a')).toBeNull()
    expect(screen.getByText(/\[Broken\]\(not a url\)/).closest('a')).toBeNull()
    expect(container.querySelector('em')).toHaveTextContent('italic')
    expect(container.querySelector('del')).toHaveTextContent('removed')
    expect(screen.getByRole('table')).toHaveTextContent('ready')
    expect(screen.getAllByRole('checkbox')).toHaveLength(2)
    expect(screen.getAllByRole('checkbox')[0]).toBeChecked()
    expect(container.querySelector('hr')).toBeInTheDocument()
    expect(container.querySelector('pre[data-language="ts"]')).toHaveTextContent('const ready')
    expect(container.querySelector('pre[data-language=""]')).toHaveTextContent('unterminated')
    expect(container.querySelector('script')).toBeNull()
  })

  it('renders an empty document', () => {
    const { container } = render(<RichText text={'\n\n'} />)
    expect(container.querySelector('.rich-text')).toBeEmptyDOMElement()
  })
})

function word(value: number): string {
  return ['zero', 'one', 'two', 'three', 'four', 'five', 'six'][value]!
}
