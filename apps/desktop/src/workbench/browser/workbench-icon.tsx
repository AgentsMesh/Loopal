export type WorkbenchIconName =
  | 'conversation' | 'federation' | 'settings' | 'sidebar'

const paths: Record<WorkbenchIconName, readonly string[]> = {
  conversation: [
    'M4.5 4.5h15v10h-8l-4.5 4v-4H4.5z',
    'M8 8h8M8 11h6',
  ],
  federation: [
    'M12 5.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z',
    'M5 17a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Zm14 0a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5Z',
    'M10.2 4.8 6.8 12m7-7.2 3.4 7.2M7.5 15h9',
  ],
  settings: [
    'M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z',
    'M19 13.2v-2.4l2-1.5-2-3.4-2.3 1a8 8 0 0 0-2-1.2L14.4 3h-4.8l-.3 2.7a8 8 0 0 0-2 1.2l-2.3-1-2 3.4 2 1.5v2.4l-2 1.5 2 3.4 2.3-1a8 8 0 0 0 2 1.2l.3 2.7h4.8l.3-2.7a8 8 0 0 0 2-1.2l2.3 1 2-3.4z',
  ],
  sidebar: ['M4 4h16v16H4z', 'M9 4v16'],
}

export function WorkbenchIcon(props: {
  readonly name: WorkbenchIconName
}): React.JSX.Element {
  return (
    <svg aria-hidden viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
      {paths[props.name].map((path) => <path d={path} key={path} />)}
    </svg>
  )
}
