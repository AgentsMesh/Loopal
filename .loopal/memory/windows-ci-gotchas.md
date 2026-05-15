---
name: Windows CI Gotchas
description: Known issues and workarounds for Windows builds with Bazel + rules_rust
type: project
created_at: 2026-04-11
updated_at: 2026-05-15
ttl_days: 90
related:
  - release-ci.md
  - codebase-scale.md
---

## PATH length limit with rules_rust

Windows has a 32,767-char limit on the PATH environment variable. On Windows, rustc adds `-Ldependency` paths to PATH for each transitive dependency. With many transitive deps (as in this project), this can exceed the limit and cause build failures.

**Known bugs:**
- bazelbuild/rules_rust#3767
- rust-lang/rust#110889

**Current repo fix:** `/Users/stone/Works/Loopal/MODULE.bazel` patches rules_rust with `/Users/stone/Works/Loopal/patches/rules_rust_windows_consolidate_deps.patch`, consolidating dependency paths so PATH stays under the Win32 limit. Safe to remove once upstream rules_rust includes the referenced fix.

**Historical workaround:** Shorten Bazel `output_base` on Windows CI (e.g., `--output_base=C:/b`) to reduce dependency path lengths. This is no longer the primary repo-local fix.
