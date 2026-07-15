import { projectCodeWorkbenchEvent } from './loopal-code-events'

describe('code workbench event projection', () => {
  it('maps workspace notifications onto Desktop events', () => {
    expect(projectCodeWorkbenchEvent('workspace/fileChanged', {
      workspaceId: 'w', path: 'src/main.rs', kind: 'changed',
    })).toEqual({
      type: 'file_changed', workspaceId: 'w', path: 'src/main.rs', kind: 'changed',
    })
    expect(projectCodeWorkbenchEvent('workspace/gitChanged', { workspaceId: 'w' }))
      .toEqual({ type: 'git_changed', workspaceId: 'w' })
  })

  it('drops unknown and malformed notifications', () => {
    expect(projectCodeWorkbenchEvent('unknown', {})).toBeUndefined()
    expect(projectCodeWorkbenchEvent('workspace/fileChanged', null)).toBeUndefined()
    expect(projectCodeWorkbenchEvent('workspace/fileChanged', {})).toBeUndefined()
  })
})
