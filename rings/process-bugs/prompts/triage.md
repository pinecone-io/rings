# Triage One And Only One Bug

You are the **triage** phase of the **bug-fixing** rings workflow.

Your job is to select the next open bug from `rings/process-bugs/queue/BUG_REPORT.md` and prepare it for
investigation. One bug per cycle — do not triage multiple bugs.

---

## Step 0: First-cycle cleanup

Delete any files in `rings/process-bugs/wip/`. Also reset any items marked `[~]`
(in-progress) back to `[ ]` in `rings/process-bugs/queue/BUG_REPORT.md` under `## Open`.
This clears state from any previously interrupted run.

---

## Step 1: Load context

Read `rings/process-bugs/queue/BUG_REPORT.md`. Find the `## Open` section.

If the `## Open` section is empty or absent (no `[ ]` or `[~]` entries), print exactly:

```
ALL_BUGS_RESOLVED
```

and stop.

---

## Step 2: Select the first open bug

Take the first entry in the `## Open` section with status `[ ]`. Skip entries marked
`[~]` — they are in-progress from an interrupted run and will have been reset in Step 0
unless this is a mid-cycle resume. One bug per cycle — do not triage multiple bugs.

Mark the selected entry as in-progress by changing `[ ]` to `[~]` in
`rings/process-bugs/queue/BUG_REPORT.md`. **Do this before writing the wip file.**

---

## Step 3: Write rings/process-bugs/wip/bug-working.md

Write `rings/process-bugs/wip/bug-working.md` with this structure:

```markdown
# Bug Working File

## Title
<the bug title, verbatim>

## Description
<the full description, verbatim>

## Severity
<Critical | High | Medium | Low — your assessment based on impact>

## Reproduction Steps
(to be filled by the fix phase)

## Root Cause
(to be filled by the fix phase)

## Fix Applied
(to be filled by the fix phase)

## Verification
(to be filled by the verify phase)
```

---

## Step 4: Print a one-line summary

Print a single line like:

```
Triaged: "<title>" → <Severity>
```
