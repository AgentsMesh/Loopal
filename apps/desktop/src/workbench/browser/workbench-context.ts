import { buildFallbackContext } from './fallback-context'
import { type useWorkbenchController } from './use-workbench-controller'

export function buildControllerContext(
  controller: ReturnType<typeof useWorkbenchController>,
): ReturnType<typeof buildFallbackContext> {
  return buildFallbackContext({
    workspaces: controller.workspaces,
    sessions: controller.projection.sessions,
    runtimes: controller.projection.runtimes,
    ...(controller.activeWorkspaceId !== undefined
      ? { activeWorkspaceId: controller.activeWorkspaceId }
      : {}),
    ...(controller.activeSessionId !== undefined
      ? { activeSessionId: controller.activeSessionId }
      : {}),
  })
}
