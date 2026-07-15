import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  type AgentControlCommand, type LoopalDesktopAPI, type SkillSummary,
} from '../../../../shared/contracts'
import { useI18n } from '../../../browser/i18n-context'
import {
  BUILTIN_SLASH_COMMANDS, BUILTIN_SLASH_NAMES, parseSlashInput,
  type SlashCommandItem, type SlashErrorCode,
} from './slash-command-model'

export type SlashSubmitResult = 'message' | 'handled' | 'blocked'
type Control = (agentId: string, command: AgentControlCommand) => Promise<boolean>

export function useSlashCommands(api: LoopalDesktopAPI, workspaceId?: string) {
  const { t } = useI18n()
  const [skills, setSkills] = useState<readonly SkillSummary[]>([])
  const [error, setError] = useState<string | undefined>(undefined)
  const [helpQuery, setHelpQuery] = useState<string | undefined>(undefined)
  const loadedWorkspace = useRef<string | undefined>(undefined)
  const requestVersion = useRef(0)

  useEffect(() => {
    requestVersion.current += 1
    loadedWorkspace.current = undefined
    setSkills([])
    setError(undefined)
    setHelpQuery(undefined)
  }, [workspaceId])

  const refresh = useCallback(async (): Promise<void> => {
    if (!workspaceId || loadedWorkspace.current === workspaceId) return
    loadedWorkspace.current = workspaceId
    const version = ++requestVersion.current
    try {
      const response = await api.listSkills(workspaceId)
      if (version === requestVersion.current) setSkills(response.skills)
    } catch {
      if (version === requestVersion.current) {
        loadedWorkspace.current = undefined
        setError(t('slash.error.skillsUnavailable'))
      }
    }
  }, [api, t, workspaceId])

  const items = useMemo<readonly SlashCommandItem[]>(() => [
    ...BUILTIN_SLASH_COMMANDS.map((command) => ({
      name: command.name,
      description: t(command.descriptionKey),
      usage: command.usage,
      arguments: command.arguments,
      source: 'runtime' as const,
      sourceLabel: t('slash.source.runtime'),
    })),
    ...skills.filter((skill) => skill.effective && !BUILTIN_SLASH_NAMES.has(skill.name))
      .map((skill) => ({
        name: skill.name,
        description: skill.description || t('slash.skillNoDescription'),
        usage: skill.hasArguments ? `${skill.name} [arguments]` : skill.name,
        arguments: skill.hasArguments ? 'optional' as const : 'none' as const,
        source: 'skill' as const,
        sourceLabel: t('slash.source.skill'),
      })),
  ], [skills, t])

  const execute = useCallback(async (
    input: string, agentId: string, control: Control, hasImages = false,
  ): Promise<SlashSubmitResult> => {
    setError(undefined)
    setHelpQuery(undefined)
    const result = parseSlashInput(input)
    if (result.kind === 'message') return 'message'
    if (result.kind === 'help') {
      setHelpQuery(result.query)
      void refresh()
      return 'handled'
    }
    if (result.kind === 'error') {
      setError(formatError(result.code, result.command, result.usage, t))
      return 'blocked'
    }
    if (hasImages) {
      setError(t('slash.error.controlWithImages'))
      return 'blocked'
    }
    return await control(agentId, result.command) ? 'handled' : 'blocked'
  }, [refresh, t])

  const clearFeedback = useCallback(() => {
    setError(undefined)
    setHelpQuery(undefined)
  }, [])

  return {
    items, error, helpQuery, refresh, execute, clearFeedback,
    dismissHelp: () => setHelpQuery(undefined),
  }
}

function formatError(
  code: SlashErrorCode, command: string, usage: string,
  t: ReturnType<typeof useI18n>['t'],
): string {
  const key = code === 'unexpected_arguments'
    ? 'slash.error.unexpectedArguments'
    : code === 'required_argument'
      ? 'slash.error.requiredArgument'
      : code === 'value_too_long'
        ? 'slash.error.valueTooLong'
        : 'slash.error.invalidValue'
  return t(key, { command, usage })
}
