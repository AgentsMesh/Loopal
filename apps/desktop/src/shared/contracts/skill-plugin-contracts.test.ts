import {
  PluginSummarySchema,
  SkillDetailSchema,
  SkillSummarySchema,
  UpsertGlobalSkillInputSchema,
} from './skill-plugin-contracts'

const revision = 'a'.repeat(64)

describe('Skill and Plugin contracts', () => {
  it('keeps legacy Unicode names displayable but not writable', () => {
    expect(SkillSummarySchema.safeParse({
      name: '/代码 审计', description: 'Legacy skill', hasArguments: false,
      source: 'project', scope: 'project', editable: false, effective: true,
    }).success).toBe(true)
    expect(UpsertGlobalSkillInputSchema.safeParse({
      workspaceId: 'workspace', name: '/代码 审计', description: '', body: 'Audit',
    }).success).toBe(false)
    expect(SkillSummarySchema.safeParse({
      name: '/nested/name', description: '', hasArguments: false,
      source: 'global', scope: 'global', editable: false, effective: true,
    }).success).toBe(false)
  })

  it('requires canonical managed names, bounded content, and exact CAS revisions', () => {
    const value = {
      workspaceId: 'workspace', name: '/review_code', description: 'Review code',
      body: 'Review $ARGUMENTS', expectedRevision: revision,
    }
    expect(UpsertGlobalSkillInputSchema.parse(value)).toEqual(value)
    expect(UpsertGlobalSkillInputSchema.safeParse({ ...value, extra: true }).success).toBe(false)
    expect(UpsertGlobalSkillInputSchema.safeParse({
      ...value, description: 'two\nlines',
    }).success).toBe(false)
    expect(UpsertGlobalSkillInputSchema.safeParse({ ...value, body: '' }).success).toBe(false)
    expect(UpsertGlobalSkillInputSchema.safeParse({
      ...value, body: '中'.repeat(35_000),
    }).success).toBe(false)
    expect(UpsertGlobalSkillInputSchema.safeParse({
      ...value, expectedRevision: 'short',
    }).success).toBe(false)
    const { expectedRevision: _expectedRevision, ...detail } = value
    expect(SkillDetailSchema.safeParse({
      ...detail, hasArguments: true, source: 'global', scope: 'global',
      editable: true, effective: true, revision,
    }).success).toBe(true)
    expect(SkillDetailSchema.safeParse({
      ...detail, body: '', hasArguments: false, source: 'global', scope: 'global',
      editable: true, effective: true, revision,
    }).success).toBe(true)
    expect(SkillDetailSchema.safeParse({
      ...detail, body: '\0', hasArguments: false, source: 'global', scope: 'global',
      editable: true, effective: true, revision,
    }).success).toBe(true)
    expect(UpsertGlobalSkillInputSchema.safeParse({ ...value, body: '\0' }).success).toBe(false)
  })

  it('accepts Unicode Plugin names but rejects path and control characters', () => {
    const plugin = {
      name: '质量工具', source: 'plugin:质量工具', skills: ['/代码 审计'],
      mcpServers: ['review'], hookCount: 1,
      hasSettings: true, hasInstructions: false, hasMemory: false,
    }
    expect(PluginSummarySchema.parse(plugin)).toEqual(plugin)
    expect(PluginSummarySchema.safeParse({ ...plugin, name: '../escape' }).success).toBe(false)
    expect(PluginSummarySchema.safeParse({ ...plugin, name: 'bad\nname' }).success).toBe(false)
  })
})
