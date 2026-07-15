import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { useState } from 'react'
import { SettingsNavigation, type SettingsSectionId } from './settings-navigation'

function Harness(): React.JSX.Element {
  const [active, setActive] = useState<SettingsSectionId>('appearance')
  const [hasResults, setHasResults] = useState(true)
  return <>
    <SettingsNavigation active={active} onSelect={setActive}
      onVisibilityChange={setHasResults} />
    <output data-testid="active-section">{active}</output>
    <output data-testid="search-results">{String(hasResults)}</output>
  </>
}

describe('SettingsNavigation', () => {
  it('groups eight second-level sections and supports pointer and keyboard selection', () => {
    render(<Harness />)
    const navigation = screen.getByTestId('settings-navigation')
    expect(within(navigation).getAllByRole('tab')).toHaveLength(8)
    for (const group of ['Desktop', 'Loopal', 'Session', 'Federation']) {
      expect(within(navigation).getByRole('heading', { name: group })).toBeInTheDocument()
    }
    const providers = within(navigation).getByRole('tab', { name: 'Model providers' })
    fireEvent.click(providers)
    expect(providers).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('active-section')).toHaveTextContent('providers')

    const appearance = within(navigation).getByRole('tab', { name: 'Desktop appearance' })
    appearance.focus()
    fireEvent.keyDown(appearance, { key: 'End' })
    const metahub = within(navigation).getByRole('tab', { name: 'MetaHub' })
    expect(metahub).toHaveFocus()
    expect(metahub).toHaveAttribute('aria-selected', 'true')
  })

  it('filters bilingual terms, selects the first result, and reports an empty search', async () => {
    render(<Harness />)
    const navigation = screen.getByTestId('settings-navigation')
    const search = within(navigation).getByRole('searchbox', { name: 'Search settings' })
    fireEvent.change(search, { target: { value: '工作区' } })
    await waitFor(() => expect(within(navigation).getAllByRole('tab')).toHaveLength(1))
    expect(within(navigation).getByRole('tab', { name: 'MCP servers' }))
      .toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('active-section')).toHaveTextContent('mcp')

    fireEvent.change(search, { target: { value: '技能' } })
    await waitFor(() => expect(within(navigation).getAllByRole('tab')).toHaveLength(1))
    expect(within(navigation).getByRole('tab', { name: 'Skills & Plugins' }))
      .toHaveAttribute('aria-selected', 'true')
    expect(screen.getByTestId('active-section')).toHaveTextContent('skills')

    fireEvent.change(search, { target: { value: 'missing-setting' } })
    await waitFor(() => expect(within(navigation).queryAllByRole('tab')).toHaveLength(0))
    expect(within(navigation).getByText('No settings sections found.')).toBeInTheDocument()
    expect(screen.getByTestId('search-results')).toHaveTextContent('false')
  })
})
