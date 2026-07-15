import { fireEvent, render, screen } from '@testing-library/react'
import {
  createStage2Callbacks, stage2Model,
} from '../../../../../test/fixtures/workbench/attention'
import { SessionAttention } from './session-attention'

describe('SessionAttention', () => {
  it('automatically renders both request kinds directly above the composer', () => {
    const callbacks = createStage2Callbacks()
    render(<SessionAttention model={stage2Model} callbacks={callbacks} />)
    const zone = screen.getByTestId('session-attention')
    expect(zone).toContainElement(screen.getByTestId('permissions-pane'))
    expect(zone).toContainElement(screen.getByTestId('questions-pane'))
    fireEvent.click(screen.getAllByRole('button', { name: 'Allow' })[0]!)
    fireEvent.click(screen.getByRole('button', { name: 'Comfortable' }))
    expect(callbacks.onResolvePermission).toHaveBeenCalled()
    expect(callbacks.onAnswerQuestion).toHaveBeenCalled()
  })

  it('occupies no session space when there are no requests', () => {
    const { container } = render(<SessionAttention
      model={{ ...stage2Model, permissions: [], questions: [] }} callbacks={{}}
    />)
    expect(container).toBeEmptyDOMElement()
  })

  it('exposes Other input, explicit submit, and request cancellation', () => {
    const callbacks = createStage2Callbacks()
    const question = {
      ...stage2Model.questions[0]!, allowMultiple: true,
      selectedChoiceIds: ['compact'], otherText: 'Custom',
      submit: { requestId: 'request', enabled: true },
    }
    render(<SessionAttention
      model={{ ...stage2Model, permissions: [], questions: [question] }} callbacks={callbacks}
    />)
    fireEvent.change(screen.getByRole('textbox', { name: /Other answer/ }), {
      target: { value: 'Typed answer' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Submit answers' }))
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(callbacks.onQuestionFreeTextChange).toHaveBeenCalledWith('style', 'Typed answer')
    expect(callbacks.onSubmitQuestionAnswers).toHaveBeenCalledWith('request')
    expect(callbacks.onCancelQuestion).toHaveBeenCalledWith('request')
  })
})
