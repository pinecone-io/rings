# Verify and Close

You are the **verify** phase of the **technical improvements** rings workflow.

Your job is to verify the implementation is correct, then remove the item from
`## Unprocessed` in `rings/process-improvements/queue/TECH_DEBT.md` and record it in
the activity log.

---

## Step 1: Load context

Read `rings/process-improvements/wip/improvement-working.md` in full, paying attention to `## Changes Made`.

---

## Step 2: Run the validation suite

Run `just validate`. If any checks fail, do **not** attempt further fixes. Instead:

1. Record the failure output under `## Verification` in `rings/process-improvements/wip/improvement-working.md`.
2. Remove the item from `## Unprocessed` in `rings/process-improvements/queue/TECH_DEBT.md`.
   Append to `rings/process-improvements/queue/BLOCKED.md` (create the file if absent):

   ```
   - [ ] **<title>**: <original description>
     → Blocked: validation failed after implementation — see git log for details
   ```

3. Delete `rings/process-improvements/wip/improvement-working.md`.
4. Print exactly:

   ```
   RINGS_CONTINUE
   ```

   and stop.

---

## Step 3: Verify no behavioral change

Confirm that the change is purely internal: no CLI output changed, no configuration
fields added or removed, no exit codes or signal semantics altered. Record this
confirmation under `## Verification` in `rings/process-improvements/wip/improvement-working.md`.

---

## Step 4: Close the item

Remove the item from `## Unprocessed` in `rings/process-improvements/queue/TECH_DEBT.md`.

Append the following to `rings/process-improvements/activities/TECH_DEBT_RESOLVED.md` (create the file if absent):

```
- [x] [YYYY-MM-DD] **<title>**: <original description>
  → <one-sentence summary of what was changed>
```

---

## Step 5: Clean up

Delete `rings/process-improvements/wip/improvement-working.md`.

Print a one-line summary:

```
Resolved: "<title>"
```
