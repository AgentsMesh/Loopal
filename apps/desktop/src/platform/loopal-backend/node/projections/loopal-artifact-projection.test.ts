import { projectModifiedFiles } from './loopal-artifact-projection'

describe('Loopal artifact projection', () => {
  it('deduplicates paths and assigns stable file types', () => {
    const artifacts = projectModifiedFiles('session', 'worker', [
      './src/main.rs', 'src/main.rs', 'README.md', 'icon.svg', 'photo.png', 'photo.jpg',
      'guide.pdf', 'notes.txt', '  ',
    ], '2026-07-11T12:00:00.000Z')
    expect(artifacts).toHaveLength(7)
    expect(artifacts.map(({ title, kind, mediaType }) => ({ title, kind, mediaType })))
      .toEqual([
        { title: 'main.rs', kind: 'code', mediaType: 'text/plain' },
        { title: 'README.md', kind: 'document', mediaType: 'text/markdown' },
        { title: 'icon.svg', kind: 'image', mediaType: 'image/svg+xml' },
        { title: 'photo.png', kind: 'image', mediaType: 'image/png' },
        { title: 'photo.jpg', kind: 'image', mediaType: 'image/jpeg' },
        { title: 'guide.pdf', kind: 'document', mediaType: 'application/pdf' },
        { title: 'notes.txt', kind: 'document', mediaType: 'text/plain' },
      ])
    expect(artifacts[0]).toMatchObject({
      sessionId: 'session', producerAgentId: 'worker',
      uri: 'loopal-workspace://src%2Fmain.rs',
      createdAt: '2026-07-11T12:00:00.000Z',
    })
    expect(projectModifiedFiles('session', 'other', ['src/main.rs'], '2026-07-11T12:00:00.000Z')[0]?.id)
      .not.toBe(artifacts[0]?.id)
  })
})
