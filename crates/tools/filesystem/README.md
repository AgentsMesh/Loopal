# filesystem/ — File & Directory Tools

Everything is a file. This category follows the Unix philosophy: tools that read,
write, search, compare, and manipulate files and directories — whether local or
remote (web).

## Local file operations

| Crate | Tool(s) | Description |
|-------|---------|-------------|
| `edit-core` | *(library)* | Shared logic: diff, search-replace, omission detection, patch parsing |
| `read` | Read | Read file contents (text, PDF, HTML→markdown) with line ranges |
| `write` | Write | Create/overwrite files, auto-create parent dirs, omission guard |
| `edit` | Edit | Precise string-replace in a file (unique match or replace-all) |
| `multi-edit` | MultiEdit | Atomic sequential edits on a single file (all-or-nothing) |
| `apply-patch` | ApplyPatch | Apply unified-diff patches across multiple files atomically |
| `diff` | Diff | Compare two files or file-vs-git-ref, unified diff output |
| `grep` | Grep | Regex content search with context, pagination, type filters |
| `glob` | Glob | Find files by glob pattern, sorted by mtime |
| `ls` | Ls | List directory contents or stat a file (permissions, size, mtime) |
| `file-ops` | CopyFile, Delete, MoveFile | Copy, delete, move/rename files and directories |

## Remote file operations

| Crate | Tool(s) | Description |
|-------|---------|-------------|
| `fetch` | Fetch | Download a URL; save to temp file or return inline (HTML→markdown) |

## Design principles

- **Path traversal protection**: all tools reject relative paths escaping cwd
- **Omission detection**: Write/Edit/MultiEdit reject LLM-generated ellipsis patterns
- **Atomic writes**: MultiEdit and ApplyPatch guarantee all-or-nothing semantics
- **URL validation**: Fetch rejects non-http(s) URLs before any network I/O
