import { z } from 'zod'

export const RelativePathSchema = z.string().max(4_096).refine(
  (value) => !value.startsWith('/') && !value.split(/[\\/]/).includes('..'),
  'path must stay inside its workspace',
)

export const FileEntrySchema = z.object({
  path: RelativePathSchema,
  name: z.string().min(1).max(255),
  kind: z.enum(['file', 'directory', 'symlink']),
  size: z.number().int().nonnegative(),
  modifiedAt: z.string().datetime().optional(),
})
export type FileEntry = z.infer<typeof FileEntrySchema>

export const DirectoryListingSchema = z.object({
  workspaceId: z.string().min(1),
  path: RelativePathSchema,
  entries: z.array(FileEntrySchema).max(10_000),
})
export type DirectoryListing = z.infer<typeof DirectoryListingSchema>

export const FileDocumentSchema = z.object({
  workspaceId: z.string().min(1),
  path: RelativePathSchema,
  content: z.string().max(10_000_000),
  version: z.string().min(1),
  languageId: z.string().min(1),
  readonly: z.boolean(),
})
export type FileDocument = z.infer<typeof FileDocumentSchema>

export const ListDirectoryInputSchema = z.object({
  workspaceId: z.string().min(1),
  path: RelativePathSchema,
})
export type ListDirectoryInput = z.infer<typeof ListDirectoryInputSchema>

export const ReadFileInputSchema = ListDirectoryInputSchema.extend({
  path: RelativePathSchema.refine((value) => value.length > 0, 'file path is required'),
})
export type ReadFileInput = z.infer<typeof ReadFileInputSchema>

export const WriteFileInputSchema = ReadFileInputSchema.extend({
  content: z.string().max(10_000_000),
  expectedVersion: z.string().min(1).nullable(),
})
export type WriteFileInput = z.infer<typeof WriteFileInputSchema>

export const SearchMatchSchema = z.object({
  path: RelativePathSchema,
  line: z.number().int().positive(),
  column: z.number().int().positive(),
  preview: z.string().max(4_000),
})
export type SearchMatch = z.infer<typeof SearchMatchSchema>

export const WorkspaceSearchInputSchema = z.object({
  workspaceId: z.string().min(1),
  query: z.string().min(1).max(1_000),
  glob: z.string().max(1_000).optional(),
  maxResults: z.number().int().positive().max(2_000).default(200),
})
export const WorkspaceSearchResultSchema = z.object({
  matches: z.array(SearchMatchSchema).max(2_000),
  truncated: z.boolean(),
})
export type WorkspaceSearchInput = z.input<typeof WorkspaceSearchInputSchema>
export type WorkspaceSearchResult = z.infer<typeof WorkspaceSearchResultSchema>

export const GitChangeSchema = z.object({
  path: RelativePathSchema,
  indexStatus: z.string().max(2),
  worktreeStatus: z.string().max(2),
})
export const GitStatusSchema = z.object({
  branch: z.string().nullable(),
  ahead: z.number().int().nonnegative(),
  behind: z.number().int().nonnegative(),
  changes: z.array(GitChangeSchema).max(100_000),
})
export type GitChange = z.infer<typeof GitChangeSchema>
export type GitStatus = z.infer<typeof GitStatusSchema>

export const WorkspaceOperationInputSchema = z.object({ workspaceId: z.string().min(1) })
export const GitDiffInputSchema = ReadFileInputSchema
export const GitStageInputSchema = ReadFileInputSchema
export const GitUnstageInputSchema = ReadFileInputSchema
export const GitDiffSchema = z.object({
  path: RelativePathSchema,
  patch: z.string().max(8 * 1024 * 1024),
  original: z.string().max(8 * 1024 * 1024),
  modified: z.string().max(8 * 1024 * 1024),
})
export type GitDiff = z.infer<typeof GitDiffSchema>
export type GitStageInput = z.infer<typeof GitStageInputSchema>
export type GitUnstageInput = z.infer<typeof GitUnstageInputSchema>

export const WorktreeSchema = z.object({
  id: z.string().min(1),
  path: z.string().min(1),
  branch: z.string().nullable(),
  head: z.string(),
  isMain: z.boolean(),
  hasChanges: z.boolean(),
})
export const WorktreeListSchema = z.array(WorktreeSchema).max(10_000)
export const CreateWorktreeInputSchema = z.object({
  workspaceId: z.string().min(1),
  name: z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/),
})
export const RemoveWorktreeInputSchema = CreateWorktreeInputSchema.extend({ force: z.boolean() })
export type Worktree = z.infer<typeof WorktreeSchema>
export type CreateWorktreeInput = z.infer<typeof CreateWorktreeInputSchema>
export type RemoveWorktreeInput = z.infer<typeof RemoveWorktreeInputSchema>

export const WorkspaceDesktopEventSchema = z.discriminatedUnion('type', [
  z.object({
    type: z.literal('file_changed'),
    workspaceId: z.string(),
    path: RelativePathSchema,
    kind: z.enum(['created', 'changed', 'deleted']),
  }),
  z.object({ type: z.literal('git_changed'), workspaceId: z.string() }),
  z.object({
    type: z.literal('workspace_resync_required'),
    workspaceId: z.string().min(1),
    reason: z.string().min(1),
  }),
])
