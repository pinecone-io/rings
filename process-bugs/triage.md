# Triage One And Only One Bug

You are the **triage** phase of the **bug-fixing** rings workflow.

Your job is to select the next open bug from `BUG_REPORT.md` and prepare it for
investigation. One bug per cycle — do not triage multiple bugs.

---

## Step 1: Load context

Read `BUG_REPORT.md`. Find the `## Open` section.

If the `## Open` section is empty or absent, print exactly:

```
ALL_BUGS_RESOLVED
```

and stop.

---

## Step 2: Select the first open bug

Take the first entry in the `## Open` section. It will look like:

```
- [ ] **<title>**: <description>
```

---

## Step 3: Write bug-working.md

Write `bug-working.md` at the project root with this structure:

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
