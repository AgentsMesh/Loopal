export type LayoutNode = SplitNode | GroupNode

export interface SplitNode {
  readonly type: 'split'
  readonly direction: 'horizontal' | 'vertical'
  readonly ratio: number
  readonly first: LayoutNode
  readonly second: LayoutNode
}

export interface GroupNode {
  readonly type: 'group'
  readonly id: string
  readonly paneIds: readonly string[]
  readonly activePaneId?: string
}

export function createDefaultLayout(): LayoutNode {
  return {
    type: 'split',
    direction: 'vertical',
    ratio: 0.72,
    first: {
      type: 'group',
      id: 'editor',
      paneIds: ['conversation', 'federation'],
      activePaneId: 'conversation',
    },
    second: {
      type: 'group',
      id: 'session',
      paneIds: [
        'artifacts', 'agents', 'tasks', 'diagnostics', 'permissions', 'questions',
      ],
      activePaneId: 'artifacts',
    },
  }
}

export function activatePane(node: LayoutNode, groupId: string, paneId: string): LayoutNode {
  if (node.type === 'group') {
    if (node.id !== groupId || !node.paneIds.includes(paneId)) {
      return node
    }
    return { ...node, activePaneId: paneId }
  }
  return {
    ...node,
    first: activatePane(node.first, groupId, paneId),
    second: activatePane(node.second, groupId, paneId),
  }
}

export function findGroup(node: LayoutNode, groupId: string): GroupNode | undefined {
  if (node.type === 'group') {
    return node.id === groupId ? node : undefined
  }
  return findGroup(node.first, groupId) ?? findGroup(node.second, groupId)
}

export function resolveActivePane(
  node: LayoutNode,
  groupId: string,
  fallback: string,
): string {
  return findGroup(node, groupId)?.activePaneId ?? fallback
}
