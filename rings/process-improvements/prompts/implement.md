# Implement the Improvement

You are the **implement** phase of the **technical improvements** rings workflow.

Your job is to execute the implementation plan in `rings/{{workflow_name}}/wip/improvement-working.md`.

---

## Step 1: Load context

Read `rings/{{workflow_name}}/wip/improvement-working.md` in full, paying close attention to
`## Implementation Plan` and `## Spec Impact Check`.

Before touching any code, re-confirm the spec impact check conclusion: this change
must not alter any user-observable behavior. If you discover mid-implementation that
a behavioral change is unavoidable, stop, record the finding under `## Changes Made` in `rings/{{workflow_name}}/wip/improvement-working.md`.
Remove the item from `## Unprocessed` in `rings/{{workflow_name}}/queue/TECH_DEBT.md`.
Append to `rings/{{workflow_name}}/queue/BLOCKED.md` (create the file if absent):

```
- [ ] **<title>**: <original description>
  → Blocked: spec impact discovered during implementation — <one sentence on what would change>
```

Delete `rings/{{workflow_name}}/wip/improvement-working.md`. Then print exactly:

```
RINGS_CONTINUE
```

and stop.

---

## Step 2: Execute the plan

Work through `## Implementation Plan` step by step. Make only the changes described
in the plan. Do not clean up surrounding code, add features, or improve things
beyond the stated scope.

---

## Step 3: Update improvement-working.md

Under `## Changes Made`, record:
- Each file changed and a one-line summary of what changed in it
- Whether any tests were added or updated
- Anything that deviated from the plan and why
