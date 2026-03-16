# Write Idea into Specification

Your job is to write the reviewed idea into the appropriate spec file and feature inventory,
then mark it as processed in rings/{{workflow_name}}/queue/IDEAS.md.

---

## Step 1: Load context

Read `rings/{{workflow_name}}/wip/idea-working.md` in full. Read the spec file listed under `## Spec File`. Read
`specs/feature_inventory.md` to find the highest existing F-NNN so you can assign the next one.

---

## Step 2: Assign a feature number

Find the highest F-NNN in `specs/feature_inventory.md`. The new feature gets the next number.

---

## Step 3: Write the spec entry

Write a prose entry in the spec file listed under `## Spec File` in `rings/{{workflow_name}}/wip/idea-working.md`.

For an **Extension**, add the new behavior as a subsection within the existing feature's section.
For a **New feature**, append a new section at the end of the file.

The entry must include:

- A clear feature name
- A 3–6 sentence description of the feature from the user's perspective
- Any configuration fields, CLI flags, or behavior changes it introduces
- Dependencies on existing features (cite F-NNN)
- Any constraints or design notes surfaced by the review panel

Apply refinements from the `## Review Synthesis` section. If the verdict was `Needs significant
rework`, incorporate the rework described there rather than the original idea.

---

## Step 4: Update feature_inventory.md

Append a new row to the appropriate table section in `specs/feature_inventory.md`:

```
| F-NNN | Feature Name | One-line user-perspective summary | BACKLOG | [spec-file.md](path/to/spec-file.md) |
```

The summary must be written from the user's perspective in the style of existing inventory entries
(e.g. "I can do X so that Y").

---

## Step 5: Record the processed idea

Remove the original idea text from `## Unprocessed` in `rings/{{workflow_name}}/queue/IDEAS.md`.

Append the following to `rings/{{workflow_name}}/activities/IDEAS_PROCESSED.md` (create the file if absent):

```
[YYYY-MM-DD] <original idea text>
→ F-NNN: <feature name>
```

---

## Step 6: Clean up and confirm

Delete `rings/{{workflow_name}}/wip/idea-working.md`.

Print a one-line confirmation:

```
Written: F-NNN — <feature name> → <spec file>
```
