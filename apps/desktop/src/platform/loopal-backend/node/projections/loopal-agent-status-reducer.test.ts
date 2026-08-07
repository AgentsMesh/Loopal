import { describe, expect, it } from 'vitest'
import { reduceAgentStatus } from './loopal-agent-status-reducer'

describe('reduceAgentStatus', () => {
  it('mirrors ordinary and terminal ViewState lifecycle transitions', () => {
    expect(reduceAgentStatus('starting', 'Started', undefined)).toBe('running')
    expect(reduceAgentStatus('running', 'AwaitingInput', undefined)).toBe('waiting')
    expect(reduceAgentStatus('running', 'Interrupted', undefined)).toBe('waiting')
    expect(reduceAgentStatus('running', 'TurnCancelled', {})).toBe('waiting')
    expect(reduceAgentStatus('running', 'Finished', undefined)).toBe('completed')
    expect(reduceAgentStatus('running', 'Error', { message: 'failed' })).toBe('failed')
    expect(reduceAgentStatus('failed', 'Finished', undefined)).toBe('failed')
  })

  it('mirrors suspend and unsuspend continuation-gate transitions', () => {
    expect(reduceAgentStatus('waiting', 'ContinuationGateChanged', {
      open: false, closed_reason: 'user_suspend',
    })).toBe('suspended')
    expect(reduceAgentStatus('suspended', 'ContinuationGateChanged', {
      open: true,
    })).toBe('running')
    expect(reduceAgentStatus('waiting', 'ContinuationGateChanged', {
      open: false, closed_reason: 'idle_timeout',
    })).toBe('waiting')
    expect(reduceAgentStatus('waiting', 'ContinuationGateChanged', null)).toBe('waiting')
  })
})
