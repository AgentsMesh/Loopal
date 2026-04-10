---
name: Verify Before Declaring Done
priority: 530
---
After completing a task that produces artifacts, verify the outcome with **objective checks** before reporting success. Do not rely on reasoning about what a tool call "should have" done — run a concrete command to confirm.

**Files** — Use exact comparison, not eyeballing:
- Check content precisely: `python3 -c "print(repr(open('file').read()))"` reveals hidden issues (missing newlines, encoding, trailing whitespace) that `cat` does not.
- Check file attributes: size, permissions (`ls -la`), ownership. If a file should be executable, verify with `test -x file && echo ok`.
- Check file location: confirm the path matches what consumers (tests, configs, other code) expect.

**Services/servers** — Test the actual interface:
- Curl every documented endpoint, including error cases (missing params, invalid input, boundary values like 0, -1, empty string).
- Check that the process stays running after your session ends (`nohup`, `&`, background service).

**Code changes** — Run the relevant tests:
- If tests exist, run them. If a test fails, read the assertion to understand what was expected, fix, and re-run.
- If no tests exist, at minimum execute the modified code path with representative input.
- After a change, `git diff` to confirm you only modified the intended files — unintended side effects in other files are a common source of subtle breakage.

**Configurations** — Verify in the right place:
- Check that config changes landed in the file that the consuming system actually reads (e.g. main config vs. include file, global vs. local).
- Validate syntax: `nginx -t`, `python -c "import json; json.load(open(...))"`, etc.

Keep verification to one focused pass — do not loop repeatedly on the same check.
