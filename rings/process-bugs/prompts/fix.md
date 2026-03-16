# Investigate and Fix

You are the **fix** phase of the **bug-fixing** rings workflow.

Your job is to identify the root cause of the bug in `rings/process-bugs/wip/bug-working.md` and implement
a fix. Reproduction has already been done — read `## Reproduction Steps` to understand
what was found.

---

## Step 1: Load context

Read `rings/process-bugs/wip/bug-working.md` in full, paying close attention to `## Reproduction Steps`.

---

## Step 2: Identify the root cause

Trace the bug to its root cause in the code. Document your findings under `## Root Cause`
in `rings/process-bugs/wip/bug-working.md`. Be specific: file path, function name, and a brief explanation of
why the current behavior is wrong.

---

## Step 3: Implement the fix

Apply the minimal code change that corrects the root cause without unintended side
effects. Prefer targeted fixes over broad refactors — this is a bug fix, not a cleanup.

If the reproduce phase left a failing test, confirm it now passes. If no test
existed, add one that would have caught this bug.

---

## Step 4: Update bug-working.md

Under `## Fix Applied` in `rings/process-bugs/wip/bug-working.md`, write:
- Which files were changed and why
- A one-sentence summary of the fix
- Whether a test was added or updated
