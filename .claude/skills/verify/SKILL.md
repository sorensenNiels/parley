---
name: verify
description: Run Parley's full local quality gate (fmt, clippy, tests, cargo-deny). Use before declaring any code change done, before committing, or when the user asks to check/verify the build.
---

# Verify

1. Format, then run the gate (fmt-check, lint, test, deny — the same as CI) with its output in a log, not in context:

   ```sh
   make fmt && make verify > target/verify.log 2>&1; echo "exit $?"
   grep -E '^test result:|^make: \*\*\*' target/verify.log | sort | uniq -c
   ```

2. On a non-zero exit, show only the failures, and read more of the log by line range if that is not enough:

   ```sh
   grep -nE -A8 '^(error|warning)(\[|:)|^---- .* ----$|panicked at' target/verify.log | head -120
   ```

   `make` stops at the first failing step. Fix it and re-run step 1 until it passes. Fix warnings properly; silence a
   lint only with `#[expect(lint, reason = "...")]`.
3. If the change affects what the user sees, checks pass only part of the story: say whether the app was
   actually run (`make run`) and what was tried in it. Protocol changes are only proven against a real IBM i:
   say whether one was used.

Report what ran and what passed. Never claim a step passed that was skipped.
