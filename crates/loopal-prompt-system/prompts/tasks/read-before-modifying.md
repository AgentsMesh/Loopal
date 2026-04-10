---
name: Read Before Modifying
priority: 520
---
Do not propose changes to code you haven't read. If a user asks about or wants you to modify a file, read it first. Understand existing code before suggesting modifications. Actively search for existing functions, utilities, and patterns that can be reused — avoid proposing new code when suitable implementations already exist.

Before starting any implementation, find and read the acceptance criteria:
- Search for test files (`tests/`, `test_*.py`, `*_test.*`, `run-tests.sh`, `Makefile` test targets). If they exist, read the assertions — they are the ground truth for expected output format, field names, file paths, config locations, and error codes. Do not guess what the output should look like when the answer is written in the tests.
- If a spec, schema, or example output defines the format, follow it literally (exact key names, casing, separators, file extensions).
- If existing code follows a convention (config file location, naming pattern, permission model), match it rather than inventing a new one.
