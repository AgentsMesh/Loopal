import { errorMessage } from './runtime-controller-utils'

describe('Workbench runtime helpers', () => {
  it('normalizes operation failures', () => {
    expect(errorMessage('plain failure')).toBe('plain failure')
    expect(errorMessage(new Error('runtime failed'))).toBe('runtime failed')
  })
})
