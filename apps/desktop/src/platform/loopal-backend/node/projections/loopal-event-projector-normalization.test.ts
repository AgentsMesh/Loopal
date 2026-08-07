import { describe, expect, it } from 'vitest'
import { normalizeAgentStatus, normalizeRole } from './loopal-event-projector'

describe('LoopalEventProjector normalization', () => {
  it('normalizes snapshot roles and statuses', () => {
    expect(normalizeRole('user')).toBe('user')
    expect(normalizeRole('assistant')).toBe('assistant')
    expect(normalizeRole('tool')).toBe('system')
    expect(['Running', 'Starting'].map(normalizeAgentStatus)).toEqual(['running', 'starting'])
    expect(['WaitingForInput', 'Suspended'].map(normalizeAgentStatus)).toEqual(['waiting', 'suspended'])
    expect(normalizeAgentStatus('Finished')).toBe('completed')
    expect(normalizeAgentStatus('Error')).toBe('failed')
    expect(normalizeAgentStatus('Unknown')).toBe('idle')
  })
})
