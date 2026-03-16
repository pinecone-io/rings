# Select and Classify One Idea

Your job is to select the next unprocessed idea from rings/process-ideas/queue/IDEAS.md, classify it against the existing
specification, and either resolve it immediately (trivial cases) or prepare it for the review panel.

---

## Step 1: Load context

Read the following files:

- `rings/process-ideas/queue/IDEAS.md` — find the `## Unprocessed` section
- `specs/feature_inventory.md` — understand what is already specified
- `specs/overview.md` — design principles and target user
- `specs/mvp.md` — original scope

If the `## Unprocessed` section is empty or absent, print exactly:

```
ALL_IDEAS_PROCESSED
```

and stop.

---

## Step 2: Select the first unprocessed idea

Take the first item in the `## Unprocessed` section. One idea per cycle — do not process multiple ideas.

---

## Step 3: Classify the idea

Classify it as exactly one of:

- **Already covered** — the idea is essentially described by an existing feature; note the F-NNN(s) and explain how.
- **Extension** — adds a new option, mode, or nuance to an existing feature; identify the F-NNN being extended.
- **New feature** — genuinely not covered; identify which spec file it would belong in.
- **Out of scope / conflict** — contradicts a design principle in `specs/overview.md` or `specs/mvp.md`; explain why.

---

## Step 4a: If Already covered or Out of scope / conflict — resolve immediately

Remove the idea from `## Unprocessed` in `rings/process-ideas/queue/IDEAS.md`.

Append the following to `rings/process-ideas/activities/IDEAS_PROCESSED.md` (create the file if absent):

```
[YYYY-MM-DD] <original idea text>
→ <classification>: <one-sentence explanation>
```

For "already covered", cite the F-NNN(s). For "out of scope", note the relevant design principle.

Then print exactly:

```
RINGS_CONTINUE
```

and stop. Do not create `idea-working.md`.

---

## Step 4b: If Extension or New feature — prepare for review

Write `idea-working.md` at the project root with the following structure:

```markdown
# Idea Working File

## Raw Idea
<the original idea text, verbatim>

## Classification
<Extension | New feature>

## Related Features
<list of F-NNN — Name — one-line summary for any directly related features, or "none">

## Spec File
<path to the spec file this belongs in, relative to project root>

## Review Synthesis
(to be filled by the review phase)
```

Then print a one-line summary of the classification, e.g.:

```
Selected: "init command to scaffold a new workflow file" → New feature (cli/commands-and-flags.md)
```
