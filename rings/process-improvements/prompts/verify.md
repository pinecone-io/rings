# Verify and Close

You are the **verify** phase of the **technical improvements** rings workflow.

Your job is to verify the implementation is correct, then move the item from
`## Unprocessed` to `## Resolved` in `queues/TECH_DEBT.md`.

---

## Step 1: Load context

Read `improvement-working.md` in full, paying attention to `## Changes Made`.

---

## Step 2: Run the validation suite

Run `just validate`. If any checks fail, do **not** attempt further fixes. Instead:

1. Record the failure output under `## Verification` in `improvement-working.md`.
2. Move the item from `## Unprocessed` to `## Blocked` in `queues/TECH_DEBT.md`:

   ```
   - [ ] **<title>**: <original description>
     → Blocked: validation failed after implementation — see improvement-working.md for details
   ```

3. Delete `improvement-working.md`.
4. Print exactly:

   ```
   RINGS_CONTINUE
   ```

   and stop.

---

## Step 3: Verify no behavioral change

Confirm that the change is purely internal: no CLI output changed, no configuration
fields added or removed, no exit codes or signal semantics altered. Record this
confirmation under `## Verification` in `improvement-working.md`.

---

## Step 4: Close the item in queues/TECH_DEBT.md

Move the item from `## Unprocessed` to `## Resolved` (create the section if absent).
Format the resolved entry as:

```
- [x] [YYYY-MM-DD] **<title>**: <original description>
  → <one-sentence summary of what was changed>
```

---

## Step 5: Clean up

Delete `improvement-working.md`.

Print a one-line summary:

```
Resolved: "<title>"
```
