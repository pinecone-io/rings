# Verify and Close

You are the **verify** phase of the **bug-fixing** rings workflow.

Your job is to verify the fix from the previous phase is correct, then remove the bug
from `## Open` in `rings/{{workflow_name}}/queue/BUG_REPORT.md` and record it in the
activity log.

---

## Step 1: Load context

Read `rings/{{workflow_name}}/wip/bug-working.md` in full, paying attention to `## Fix Applied`.

---

## Step 2: Run the test suite

Run `just validate`. If any checks fail, do **not** attempt further fixes. Instead:

1. Record the failure output under `## Verification` in `rings/{{workflow_name}}/wip/bug-working.md`.
2. Remove the bug entry from `## Open` in `rings/{{workflow_name}}/queue/BUG_REPORT.md`.
   Append to `rings/{{workflow_name}}/queue/BLOCKED.md` (create the file if absent):

   ```
   - [ ] **<title>**: <original description>
     → Blocked: tests failed after fix — see git log for details
   ```

3. Delete `rings/{{workflow_name}}/wip/bug-working.md`.
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
- The fix matches the stated root cause in `rings/{{workflow_name}}/wip/bug-working.md`

Document your verification result under `## Verification` in `rings/{{workflow_name}}/wip/bug-working.md`.

---

## Step 4: Close the bug

Remove the bug entry from `## Open` in `rings/{{workflow_name}}/queue/BUG_REPORT.md`.

Append the following to `rings/{{workflow_name}}/activities/BUGS_RESOLVED.md` (create the file if absent):

```
- [x] [YYYY-MM-DD] **<title>**: <original description>
  → Fixed: <one-sentence summary of the fix from bug-working.md>
```

---

## Step 5: Clean up

Delete `rings/{{workflow_name}}/wip/bug-working.md` — it is no longer needed.

Print a one-line summary:

```
Resolved: "<title>"
```
