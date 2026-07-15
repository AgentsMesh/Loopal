import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { createTestAPI } from '../../../../../test/support/workbench/api-stub'
import { type SkillDetail, type SkillSummary } from '../../../../shared/contracts'
import { LoopalSkillPluginSettings } from './loopal-skill-plugin-settings'

const revision = 'a'.repeat(64)
const globalSkill: SkillSummary = {
  name: '/review', description: 'Review globally', hasArguments: true,
  source: 'global', scope: 'global', editable: true, effective: false, revision,
}
const projectSkill: SkillSummary = {
  name: '/review', description: 'Project review policy', hasArguments: true,
  source: 'project', scope: 'project', editable: false, effective: true,
}
const detail: SkillDetail = {
  workspaceId: 'workspace', ...globalSkill, body: 'Review $ARGUMENTS', revision,
}

function harness(overrides: Parameters<typeof createTestAPI>[0] = {}) {
  const listSkills = vi.fn(async (workspaceId: string) => ({
    workspaceId, skills: [globalSkill, projectSkill],
  }))
  const listPlugins = vi.fn(async (workspaceId: string) => ({
    workspaceId, plugins: [{
      name: 'quality', source: 'plugin:quality', skills: ['/audit'],
      mcpServers: ['reviewer'], hookCount: 2,
      hasSettings: true, hasInstructions: true, hasMemory: false,
    }],
  }))
  const getSkill = vi.fn(async () => detail)
  const upsertGlobalSkill = vi.fn(async (input) => ({
    workspaceId: input.workspaceId, name: input.name, description: input.description,
    body: input.body, hasArguments: input.body.includes('$ARGUMENTS'),
    source: 'global', scope: 'global' as const, editable: true, effective: true,
    revision: 'b'.repeat(64),
  }))
  const deleteGlobalSkill = vi.fn(async (input) => ({
    workspaceId: input.workspaceId, skills: [projectSkill],
  }))
  const { api } = createTestAPI({
    listSkills, listPlugins, getSkill, upsertGlobalSkill, deleteGlobalSkill, ...overrides,
  })
  render(<LoopalSkillPluginSettings api={api} workspaceId="workspace" />)
  return { listSkills, listPlugins, getSkill, upsertGlobalSkill, deleteGlobalSkill }
}

describe('LoopalSkillPluginSettings', () => {
  it('distinguishes no-workspace and loading states', () => {
    const fallback = createTestAPI()
    const first = render(<LoopalSkillPluginSettings api={fallback.api} />)
    expect(screen.getByText('Open a live Session to inspect its project Skills and Plugins.'))
      .toBeInTheDocument()
    first.unmount()
    const { api } = createTestAPI({
      listSkills: async () => new Promise(() => undefined),
      listPlugins: async () => new Promise(() => undefined),
    })
    render(<LoopalSkillPluginSettings api={api} workspaceId="workspace" />)
    expect(screen.getByRole('status')).toHaveTextContent('Loading Skills and Plugins…')
  })

  it('shows all global definitions, effective provenance, and Plugin contributions', async () => {
    harness()
    const root = await screen.findByTestId('skills-plugin-settings')
    expect(await within(root).findByTestId('global-skill-review'))
      .toHaveTextContent('Overridden by this project')
    const effective = within(root).getByTestId('effective-skill-list')
    expect(effective).toHaveTextContent('/review')
    expect(effective).toHaveTextContent('Source · project')
    const plugin = within(root).getByTestId('plugin-quality')
    expect(plugin).toHaveTextContent('Skills · /audit')
    expect(plugin).toHaveTextContent('MCP servers · reviewer')
    expect(plugin).toHaveTextContent('settings.json, LOOPAL.md')
    expect(root).toHaveTextContent('Skill edits apply to the next /name invocation.')
    expect(root).toHaveTextContent('require a Session restart')
  })

  it('creates a canonical global Skill without inventing a revision', async () => {
    const actions = harness()
    fireEvent.click(await screen.findByTestId('skill-create'))
    fireEvent.change(screen.getByTestId('skill-name'), { target: { value: '/ship' } })
    fireEvent.change(screen.getByTestId('skill-description'), {
      target: { value: 'Prepare a release' },
    })
    fireEvent.change(screen.getByTestId('skill-body'), {
      target: { value: 'Ship $ARGUMENTS safely' },
    })
    const save = screen.getByTestId('skill-save')
    expect(save).toBeEnabled()
    fireEvent.click(save)
    await waitFor(() => expect(actions.upsertGlobalSkill).toHaveBeenCalledWith({
      workspaceId: 'workspace', name: '/ship', description: 'Prepare a release',
      body: 'Ship $ARGUMENTS safely',
    }))
    expect(await screen.findByRole('status')).toHaveTextContent('Saved /ship')
    expect(screen.getByTestId('skill-name')).toBeDisabled()
  })

  it('loads exact global content and requires confirmation before CAS delete', async () => {
    const actions = harness()
    fireEvent.click(await screen.findByRole('button', { name: 'Edit /review' }))
    expect(await screen.findByDisplayValue('Review $ARGUMENTS')).toBeInTheDocument()
    expect(actions.getSkill).toHaveBeenCalledWith({ workspaceId: 'workspace', name: '/review' })
    fireEvent.click(screen.getByTestId('skill-delete'))
    const confirmation = screen.getByRole('alertdialog')
    expect(confirmation).toHaveTextContent('Delete /review?')
    expect(actions.deleteGlobalSkill).not.toHaveBeenCalled()
    fireEvent.click(within(confirmation).getByRole('button', { name: 'Delete permanently' }))
    await waitFor(() => expect(actions.deleteGlobalSkill).toHaveBeenCalledWith({
      workspaceId: 'workspace', name: '/review', expectedRevision: revision,
    }))
    expect(await screen.findByRole('status')).toHaveTextContent('Deleted /review')
  })

  it('preserves the draft and exposes a stale-revision failure as an alert', async () => {
    const upsertGlobalSkill = vi.fn(async () => {
      throw new Error('Skill changed on disk; reload before editing')
    })
    harness({ upsertGlobalSkill })
    fireEvent.click(await screen.findByRole('button', { name: 'Edit /review' }))
    const body = await screen.findByTestId('skill-body')
    fireEvent.change(body, { target: { value: 'Unsaved local draft' } })
    fireEvent.click(screen.getByTestId('skill-save'))
    expect(await screen.findByRole('alert')).toHaveTextContent('changed on disk')
    expect(screen.getByTestId('skill-body')).toHaveValue('Unsaved local draft')
  })
})
