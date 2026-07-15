import { type QuestionRequest } from '../../../../shared/contracts'

type Question = QuestionRequest['questions'][number]

export interface QuestionAnswerDraft {
  readonly selected: readonly string[]
  readonly other: string
}

export function questionDrafts(
  request: QuestionRequest,
  current: readonly QuestionAnswerDraft[] | undefined,
): QuestionAnswerDraft[] {
  return request.questions.map((_question, index) => ({
    selected: [...(current?.[index]?.selected ?? [])],
    other: current?.[index]?.other ?? '',
  }))
}

export function chooseAnswer(
  question: Question,
  draft: QuestionAnswerDraft,
  choiceId: string,
): QuestionAnswerDraft {
  if (!question.allowMultiple) return { selected: [choiceId], other: '' }
  const selected = draft.selected.includes(choiceId)
    ? draft.selected.filter((value) => value !== choiceId)
    : [...draft.selected, choiceId]
  return { ...draft, selected }
}

export function writeOther(
  question: Question,
  draft: QuestionAnswerDraft,
  value: string,
): QuestionAnswerDraft {
  return question.allowMultiple
    ? { ...draft, other: value }
    : { selected: [], other: value }
}

export function questionAnswers(
  request: QuestionRequest,
  drafts: readonly QuestionAnswerDraft[] | undefined,
): string[] | undefined {
  const answers = request.questions.map((question, index) => {
    const draft = drafts?.[index] ?? { selected: [], other: '' }
    const labels = draft.selected.map(choiceAnswer)
    const other = draft.other.trim()
    if (!question.allowMultiple) return other || labels[0] || ''
    return [...labels, ...(other ? [other] : [])].join(', ')
  })
  return answers.every(Boolean) ? answers : undefined
}

export function choiceAnswer(choiceId: string): string {
  return choiceId.slice(choiceId.indexOf(':') + 1)
}
