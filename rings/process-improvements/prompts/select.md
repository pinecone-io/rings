# Select and Classify One Improvement

You are the **select** phase of the **technical improvements** rings workflow.

Your job is to select the next unprocessed item from `rings/process-improvements/queue/TECH_DEBT.md`, classify it,
and either resolve it immediately (trivial cases) or prepare it for planning.

---

## Step 1: Load context

Read `rings/process-improvements/queue/TECH_DEBT.md`. Find the `## Unprocessed` section.

If the `## Unprocessed` section is empty or absent, print exactly:

```
ALL_IMPROVEMENTS_PROCESSED
```

and stop.

---

## Step 2: Select the first unprocessed item

Take the first entry in `## Unprocessed`. One item per cycle — do not process multiple.

---

## Step 3: Classify the item

Read `specs/feature_inventory.md` and `specs/overview.md` to understand what is
already specified and what the design principles are.

Classify the item as exactly one of:

- **Valid** — a genuine internal improvement (refactor, dedup, dependency change,
  performance, test coverage, tooling) that does not add, remove, or change any
  product behavior described in `specs/`.
- **Already done** — the improvement has already been made; the codebase reflects it.
- **Out of scope** — the change would require adding, removing, or altering product
  behavior described in `specs/`. This should go through `process-ideas.rings.toml`
  instead.

---

## Step 4a: If Already done or Out of scope — resolve immediately

Remove the item from `## Unprocessed` in `rings/process-improvements/queue/TECH_DEBT.md`.

Append the following to `rings/process-improvements/activities/TECH_DEBT_RESOLVED.md` (create the file if absent):

```
[YYYY-MM-DD] <original item text>
→ <classification>: <one-sentence explanation>
```

For "out of scope", note that it should be filed in `rings/process-ideas/queue/IDEAS.md` if it represents a
desired feature change.

Then print exactly:

```
RINGS_CONTINUE
```

and stop. Do not create `improvement-working.md`.

---

## Step 4b: If Valid — prepare for planning

Write `improvement-working.md` at the project root with this structure:

```markdown
# Improvement Working File

## Title
<the item title, verbatim>

## Description
<the full description, verbatim>

## Category
<Refactor | Deduplication | Dependency | Performance | Test Coverage | Tooling | Other>

## Motivation
<one or two sentences on why this makes rings better to work on or more reliable>

## Spec Impact Check
(to be filled by the plan phase)

## Implementation Plan
(to be filled by the plan phase)

## Changes Made
(to be filled by the implement phase)

## Verification
(to be filled by the verify phase)
```

Then print a one-line summary:

```
Selected: "<title>" → <Category>
```
