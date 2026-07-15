import { type PluginSummary, type SkillSummary } from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'

export function GlobalSkillList(props: {
  readonly skills: readonly SkillSummary[]
  readonly busy: boolean
  readonly onEdit: (name: string) => void
}): React.JSX.Element {
  const { t } = useI18n()
  const skills = props.skills.filter((skill) => skill.scope === 'global')
  return <section className="skill-list-section" aria-labelledby="global-skills-title">
    <h4 id="global-skills-title">{t('settings.skills.global.title')}</h4>
    <p className="muted">{t('settings.skills.global.help')}</p>
    <div className="skill-card-list" data-testid="global-skill-list" role="list">
      {!skills.length && <p className="muted">{t('settings.skills.global.empty')}</p>}
      {skills.map((skill) => <article className="skill-card"
        role="listitem"
        data-testid={`global-skill-${testId(skill.name)}`} key={`${skill.source}:${skill.name}`}>
        <div><strong>{skill.name}</strong><p>{skill.description}</p></div>
        <small>{t(skill.effective
          ? 'settings.skills.global.effective' : 'settings.skills.global.overridden')}</small>
        {skill.editable && <button type="button" disabled={props.busy}
          aria-label={t('settings.skills.global.edit', { name: skill.name })}
          onClick={() => props.onEdit(skill.name)}>{t('settings.skills.global.edit', {
            name: skill.name,
          })}</button>}
      </article>)}
    </div>
  </section>
}

export function EffectiveSkillList(props: {
  readonly skills: readonly SkillSummary[]
}): React.JSX.Element {
  const { t } = useI18n()
  const skills = props.skills.filter((skill) => skill.effective)
  return <section className="skill-list-section" aria-labelledby="effective-skills-title">
    <h4 id="effective-skills-title">{t('settings.skills.effective.title')}</h4>
    <p className="muted">{t('settings.skills.effective.help')}</p>
    <div className="skill-card-list" data-testid="effective-skill-list" role="list">
      {!skills.length && <p className="muted">{t('settings.skills.effective.empty')}</p>}
      {skills.map((skill) => <article className="skill-card"
        role="listitem"
        key={`${skill.source}:${skill.name}`}>
        <div><strong>{skill.name}</strong><p>{skill.description}</p></div>
        <small>{t('settings.skills.source', { source: skill.source })}</small>
        <span>{t(skill.hasArguments
          ? 'settings.skills.arguments' : 'settings.skills.noArguments')}</span>
      </article>)}
    </div>
  </section>
}

export function PluginList(props: {
  readonly plugins: readonly PluginSummary[]
}): React.JSX.Element {
  const { t } = useI18n()
  return <section className="skill-list-section plugin-list-section"
    aria-labelledby="plugins-title">
    <h4 id="plugins-title">{t('settings.plugins.title')}</h4>
    <p className="muted">{t('settings.plugins.help')}</p>
    <p className="plugin-restart-note">{t('settings.plugins.restart')}</p>
    <div className="plugin-card-list" data-testid="plugin-list" role="list">
      {!props.plugins.length && <p className="muted">{t('settings.plugins.empty')}</p>}
      {props.plugins.map((plugin) => <article className="plugin-card"
        role="listitem"
        data-testid={`plugin-${testId(plugin.name)}`} key={plugin.name}>
        <strong>{plugin.name}</strong><small>{plugin.source}</small>
        <p>{t('settings.plugins.skills', { items: list(plugin.skills, t('settings.plugins.none')) })}</p>
        <p>{t('settings.plugins.mcp', { items: list(plugin.mcpServers, t('settings.plugins.none')) })}</p>
        <p>{t('settings.plugins.hooks', { count: plugin.hookCount })}</p>
        <p>{t('settings.plugins.files', { items: pluginFiles(plugin, t) })}</p>
      </article>)}
    </div>
  </section>
}

function pluginFiles(plugin: PluginSummary, t: ReturnType<typeof useI18n>['t']): string {
  const values = [
    plugin.hasSettings ? t('settings.plugins.settings') : undefined,
    plugin.hasInstructions ? t('settings.plugins.instructions') : undefined,
    plugin.hasMemory ? t('settings.plugins.memory') : undefined,
  ].filter((value): value is string => Boolean(value))
  return list(values, t('settings.plugins.none'))
}

function list(values: readonly string[], empty: string): string {
  return values.length ? values.join(', ') : empty
}

function testId(value: string): string {
  return value.replace(/^\//, '').replaceAll(/[^A-Za-z0-9_-]/g, '-')
}
