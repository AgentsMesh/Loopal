---
name: Windows CI Gotchas
description: Known issues and workarounds for Windows builds with Bazel + rules_rust
type: project
---

## PATH length limit with rules_rust

Windows has a 32,767-char limit on the PATH environment variable. On Windows, rustc adds `-Ldependency` paths to PATH for each transitive dependency. With many transitive deps (as in this project), this can exceed the limit and cause build failures.

**Known bugs:**
- bazelbuild/rules_rust#3767
- rust-lang/rust#110889

**Workaround:** Shorten the Bazel `output_base` path on Windows CI (e.g., `--output_base=C:/b`) to reduce the length of each dependency path added to PATH.
