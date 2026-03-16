# Verify and Close

You are the **verify** phase of the **bug-fixing** rings workflow.

Your job is to verify the fix from the previous phase is correct, then move the bug
from `## Open` to `## Resolved` in `rings/process-bugs/queue/BUG_REPORT.md`.

---

## Step 1: Load context

Read `bug-working.md` in full, paying attention to `## Fix Applied`.

---

## Step 2: Run the test suite

Run `just validate`. If any checks fail, do **not** attempt further fixes. Instead:

1. Record the failure output under `## Verification` in `bug-working.md`.
2. Remove the bug entry from `## Open` in `rings/process-bugs/queue/BUG_REPORT.md`.
   Append to `rings/process-bugs/queue/BLOCKED.md` (create the file if absent):

   ```
   - [ ] **<title>**: <original description>
     → Blocked: tests failed after fix — see git log for details
   ```

3. Delete `bug-working.md`.
4. Print exactly:

   ```
   RINGS_CONTINUE
   ```

   and stop. Do not close the bug.

---

## Step 3: Verify the fix

Confirm that:
- The specific behavior described in the bug no longer occurs
- No regressions were introduced (the test suite is clean)
- The fix matches the stated root cause in `bug-working.md`

Document your verification result under `## Verification` in `bug-working.md`.

---

## Step 4: Close the bug

Remove the bug entry from `## Open` in `rings/process-bugs/queue/BUG_REPORT.md`.

Append the following to `rings/process-bugs/activities/BUGS_RESOLVED.md` (create the file if absent):

```
- [x] [YYYY-MM-DD] **<title>**: <original description>
  → Fixed: <one-sentence summary of the fix from bug-working.md>
```

---

## Step 5: Clean up

Delete `bug-working.md` — it is no longer needed.

Print a one-line summary:

```
Resolved: "<title>"
```
