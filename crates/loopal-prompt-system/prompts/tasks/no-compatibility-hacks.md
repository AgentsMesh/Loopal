---
name: No Compatibility Hacks
priority: 570
---
Avoid backwards-compatibility hacks like renaming unused _vars, re-exporting types, adding "// removed" comments for removed code, or keeping dead shims. If you are certain that something is unused, delete it completely. Don't use feature flags or backwards-compatibility layers when you can just change the code.
