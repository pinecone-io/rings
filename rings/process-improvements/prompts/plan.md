# Plan the Improvement

You are the **plan** phase of the **technical improvements** rings workflow.

Your job is to design a concrete implementation plan for the improvement in
`improvement-working.md` and confirm it does not touch any product behavior
described in `specs/`.

---

## Step 1: Load context

Read `improvement-working.md` in full. Read `specs/feature_inventory.md` to orient
yourself on what is specified. Read any spec files directly relevant to the area
being changed.

---

## Step 2: Spec impact check

Carefully determine whether the proposed change would alter, add, or remove any
behavior that a user of rings could observe — CLI output, file formats, exit codes,
configuration fields, signal semantics, or anything else described in `specs/`.

Record your findings under `## Spec Impact Check` in `improvement-working.md`:

- **No spec impact** — describe in one sentence why the change is purely internal.
- **Spec impact detected** — describe exactly which spec behavior would be affected.

If spec impact is detected, this improvement is out of scope for this workflow. Remove
the item from `## Unprocessed` in `rings/process-improvements/queue/TECH_DEBT.md`.

Append the following to `rings/process-improvements/activities/TECH_DEBT_RESOLVED.md` (create the file if absent):

```
[YYYY-MM-DD] <original item text>
→ Out of scope (spec impact): <one-sentence explanation of what would change>
→ Suggest filing in rings/process-ideas/queue/IDEAS.md if a product change is desired.
```

Delete `improvement-working.md`. Then print exactly:

```
RINGS_CONTINUE
```

and stop.

---

## Step 3: Write the implementation plan

Under `## Implementation Plan` in `improvement-working.md`, write a step-by-step
plan covering:

- Which files will be changed and why
- The order of changes (if order matters for correctness or reviewability)
- Any risks or subtleties (e.g. trait bound implications, test isolation, semver)
- How to verify the change is correct beyond `just validate` passing (if applicable)

Keep the plan concrete and actionable. The implement phase will execute it directly.

---

## Step 4: Print a one-line summary

```
Planned: "<title>" — <number of files affected> file(s) to change
```
