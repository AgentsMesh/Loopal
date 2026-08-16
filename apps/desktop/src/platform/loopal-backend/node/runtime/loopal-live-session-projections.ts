import {
  type SessionDetail, type SessionView, type WorkflowRunSummary,
} from '../../../../shared/contracts'
import { projectModifiedFiles } from '../projections/loopal-artifact-projection'
import { reduceWorkflowRun } from '../projections/loopal-workflow-projection'

export function mergeLiveArtifacts(
  detail: SessionDetail,
  sessionId: string,
  agentId: string,
  paths: readonly string[],
  createdAt: string,
) {
  const known = new Set(detail.artifacts.map((artifact) => artifact.id))
  const created = projectModifiedFiles(sessionId, agentId, paths, createdAt)
    .filter((artifact) => !known.has(artifact.id))
  if (created.length === 0) return undefined
  return { detail: { ...detail, artifacts: [...detail.artifacts, ...created] }, created }
}

type LiveWorkflowResult =
  | { kind: 'noop' }
  | { kind: 'gap' }
  | { kind: 'applied'; detail: SessionDetail }

export function reduceLiveWorkflow(
  detail: SessionDetail | undefined,
  summary: WorkflowRunSummary,
): LiveWorkflowResult {
  const view = detail?.view
  if (!detail || !view) return { kind: 'noop' }
  const result = reduceWorkflowRun(view.workflows, summary)
  if (result.kind !== 'applied') return result
  const update = (current: SessionView): SessionView => ({
    ...current, workflows: result.workflows,
  })
  return {
    kind: 'applied',
    detail: {
      ...detail,
      view: update(view),
      agents: detail.agents.map((agent) => (
        agent.id === 'main' && agent.view ? { ...agent, view: update(agent.view) } : agent
      )),
    },
  }
}
