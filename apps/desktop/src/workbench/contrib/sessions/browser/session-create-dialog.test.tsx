import { useState } from 'react'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { type SessionDirectorySelection } from '../../../../shared/contracts'
import { SessionCreateDialog } from './session-create-dialog'

const authorizationId = 'd10f67f2-f471-44ea-b6d1-e1b963e11228'
const plain: SessionDirectorySelection = {
  authorizationId, path: '/work/notes', name: 'notes', suggestedWorktreeName: 'notes-task',
}
const git: SessionDirectorySelection = {
  authorizationId, path: '/work/loopal', name: 'loopal', suggestedWorktreeName: 'loopal-task',
  git: { root: '/work/loopal', branch: 'main', dirty: true },
}
const cleanGit: SessionDirectorySelection = {
  ...git, git: { ...git.git!, dirty: false },
}

describe('SessionCreateDialog', () => {
  it('requires an authorized directory and does nothing when selection is cancelled', async () => {
    const selectSessionDirectory = vi.fn(async () => undefined)
    const onCreate = vi.fn(async () => undefined)
    render(<SessionCreateDialog api={{ selectSessionDirectory }}
      onCreate={onCreate} onClose={vi.fn()} />)

    expect(screen.getByTestId('create-session-confirm')).toBeDisabled()
    fireEvent.click(screen.getByTestId('session-directory'))
    await waitFor(() => expect(selectSessionDirectory).toHaveBeenCalledOnce())
    expect(screen.getByTestId('create-session-confirm')).toBeDisabled()
    expect(onCreate).not.toHaveBeenCalled()
  })

  it('creates a direct session from a regular directory', async () => {
    const onCreate = vi.fn(async () => undefined)
    const onClose = vi.fn()
    render(<SessionCreateDialog api={{ selectSessionDirectory: async () => plain }}
      onCreate={onCreate} onClose={onClose} />)

    fireEvent.click(screen.getByTestId('session-directory'))
    await screen.findByText('/work/notes')
    expect(screen.getByTestId('launch-direct')).toBeChecked()
    expect(screen.queryByTestId('launch-worktree')).not.toBeInTheDocument()
    fireEvent.click(screen.getByTestId('create-session-confirm'))
    await waitFor(() => expect(onCreate).toHaveBeenCalledWith({
      authorizationId, launchMode: 'directory',
    }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('validates a Git worktree and explains dirty HEAD behavior', async () => {
    const onCreate = vi.fn(async () => undefined)
    render(<SessionCreateDialog api={{ selectSessionDirectory: async () => git }}
      onCreate={onCreate} onClose={vi.fn()} />)

    fireEvent.click(screen.getByTestId('session-directory'))
    await screen.findByText('Git repository')
    fireEvent.click(screen.getByTestId('launch-worktree'))
    expect(screen.getByTestId('worktree-name')).toHaveValue('loopal-task')
    expect(screen.getByText(/uncommitted changes are not copied/i)).toBeInTheDocument()
    fireEvent.change(screen.getByTestId('worktree-name'), { target: { value: 'bad name' } })
    expect(screen.getByTestId('create-session-confirm')).toBeDisabled()
    fireEvent.change(screen.getByTestId('worktree-name'), { target: { value: 'issue_42' } })
    fireEvent.click(screen.getByTestId('create-session-confirm'))
    await waitFor(() => expect(onCreate).toHaveBeenCalledWith({
      authorizationId, launchMode: 'worktree', worktreeName: 'issue_42',
    }))
  })

  it('always explains that worktrees start at HEAD', async () => {
    render(<SessionCreateDialog api={{ selectSessionDirectory: async () => cleanGit }}
      onCreate={async () => undefined} onClose={vi.fn()} />)
    fireEvent.click(screen.getByTestId('session-directory'))
    await screen.findByText('Git repository')
    fireEvent.click(screen.getByTestId('launch-worktree'))
    expect(screen.getByText(/uncommitted changes are not copied/i)).toBeInTheDocument()
  })

  it('keeps the dialog open and renders create failures inline', async () => {
    render(<SessionCreateDialog api={{ selectSessionDirectory: async () => plain }}
      onCreate={async () => 'directory_selection_invalid'} onClose={vi.fn()} />)
    fireEvent.click(screen.getByTestId('session-directory'))
    await screen.findByText('/work/notes')
    fireEvent.click(screen.getByTestId('create-session-confirm'))
    expect(await screen.findByTestId('session-create-error'))
      .toHaveTextContent('directory_selection_invalid')
    expect(screen.getByTestId('new-session-dialog')).toBeInTheDocument()
  })

  it('traps initial focus and restores the trigger on Escape', async () => {
    function Harness(): React.JSX.Element {
      const [open, setOpen] = useState(false)
      return <><button onClick={() => setOpen(true)}>Open wizard</button>
        {open && <SessionCreateDialog api={{ selectSessionDirectory: async () => plain }}
          onCreate={async () => undefined} onClose={() => setOpen(false)} />}</>
    }
    render(<Harness />)
    const trigger = screen.getByRole('button', { name: 'Open wizard' })
    trigger.focus()
    fireEvent.click(trigger)
    await waitFor(() => expect(screen.getByTestId('session-directory')).toHaveFocus())
    fireEvent.keyDown(screen.getByTestId('new-session-dialog'), { key: 'Escape' })
    await waitFor(() => expect(trigger).toHaveFocus())
    expect(screen.queryByTestId('new-session-dialog')).not.toBeInTheDocument()
  })

  it('keeps worktree input focus across unrelated parent rerenders', async () => {
    const api = { selectSessionDirectory: async () => git }
    const onCreate = async (): Promise<undefined> => undefined
    const view = render(<SessionCreateDialog api={api}
      onCreate={onCreate} onClose={() => undefined} />)
    fireEvent.click(screen.getByTestId('session-directory'))
    await screen.findByText('Git repository')
    fireEvent.click(screen.getByTestId('launch-worktree'))
    const input = screen.getByTestId('worktree-name')
    input.focus()

    view.rerender(<SessionCreateDialog api={api}
      onCreate={onCreate} onClose={() => undefined} />)

    expect(input).toHaveFocus()
  })
})
