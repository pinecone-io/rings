# Verify and Close

You are the **verify** phase of the **bug-fixing** rings workflow.

Your job is to verify the fix from the previous phase is correct, then move the bug
from `## Open` to `## Resolved` in `queues/BUG_REPORT.md`.

---

## Step 1: Load context

Read `bug-working.md` in full, paying attention to `## Fix Applied`.

---

## Step 2: Run the test suite

Run `just validate`. If any checks fail, do **not** attempt further fixes. Instead:

1. Record the failure output under `## Verification` in `bug-working.md`.
2. Move the bug entry from `## Open` to `## Blocked` in `queues/BUG_REPORT.md` (create the
   section if absent). Format the entry as:

   ```
   - [ ] **<title>**: <original description>
     → Blocked: tests failed after fix — see bug-working.md for details
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

## Step 4: Close the bug in queues/BUG_REPORT.md

Move the bug entry from `## Open` to `## Resolved` (create the section if absent).
Format the resolved entry as:

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
