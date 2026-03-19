Draft and review an implementation plan for the next batch of prioritized features, then write it to TODO.md.

## Steps

### 1. Select the batch

Read `specs/feature_inventory.md`. Select the next 5–10 features that have status `PRIORITIZED`.

Use these grouping criteria:
1. **Priority order** — lower feature numbers first (generally)
2. **Logical grouping** — prefer features sharing a spec file or implementation surface
3. **Coherent scope** — the batch should be reviewable and implementable together

For each selected feature, read its spec file in full. Show the user the proposed batch and wait for approval before continuing.

### 2. Draft the plan

Using the Agent tool, launch the following 3 agents **in parallel** (single message, 3 tool calls):

Agents: `impl-rust`, `impl-architecture`, `impl-deps`

Prompt each with:

> *"You are a member of the rings initial planning panel. Orient yourself:*
> - *Read `specs/index.md` and `specs/overview.md` to understand what rings is*
> - *Read the relevant source files in `src/` that relate to your area of expertise*
> - *Read the spec files listed below for the selected features*
>
> *These features are selected for the next implementation batch. Produce an initial technical plan from your area of expertise. For each feature, identify: source files to create or modify, key types/traits/structs to introduce, test cases required (unit and integration), and any cross-feature dependencies.*
>
> *Selected features and their specs: [list F-NNN · Name · spec file for each]*
>
> *Return your findings as structured markdown."*

### 3. Review the draft

Using the Agent tool, launch the following 7 agents **in parallel** (single message, 7 tool calls):

Agents: `impl-testing`, `impl-error-handling`, `impl-cli-framework`, `impl-serialization`, `impl-process-mgmt`, `impl-filesystem`, `impl-agent-ux`

Prompt each with:

> *"You are a member of the rings implementation review panel. Orient yourself:*
> - *Read `specs/index.md` and `specs/overview.md` to understand what rings is*
> - *Read the relevant source files in `src/` that relate to your area of expertise*
> - *Read the spec files listed below for the selected features*
>
> *The following features are selected for the next implementation batch. A draft technical plan has already been produced. Do a full implementation review from your area of expertise. For each feature, identify:*
> 1. *Prerequisite work — missing abstractions, dependencies, or data model changes that must come first*
> 2. *Design decisions — explicit choices with meaningful tradeoffs; include a recommended default*
> 3. *Test cases — specific cases that must be covered*
> 4. *Spec gaps — ambiguities that would affect implementation*
>
> *Selected features and their specs: [list F-NNN · Name · spec file for each]*
>
> *Draft plan: [full merged draft from Step 2]*
>
> *Return numbered findings grouped by the four categories above. Skip categories with no findings."*

### 4. Synthesize

Consolidate all 10 agents' outputs. Discard inapplicable findings; merge duplicates.

Produce four sections:
- **Implementation Steps** — ordered, dependency-aware list. Prerequisite work (new abstractions, deps, data model changes) goes first. Each step names files to touch, types/traits/functions to add, and test cases required.
- **Open Decisions** — explicit choices with tradeoffs and recommended defaults.
- **Test Requirements** — test cases not already captured in steps.
- **Spec Gaps** — ambiguities the implementer should note or resolve.

Show the synthesized plan to the user and wait for approval before writing.

### 5. Write to TODO.md (after user confirms)

Append the finalized plan to `TODO.md` at the repo root:

```markdown
## F-NNN: Feature Name

**Spec:** `specs/path/to/spec.md`

### Task 1: <short title>

**Files:** <files to create or modify>

**Steps:**
- [ ] <concrete implementation step>

**Tests:**
- [ ] <test case>

[repeat Task N blocks; prerequisite tasks first]

---
```

Then update `specs/feature_inventory.md`: change each feature's status from `PRIORITIZED` to `PLANNED`.

Confirm to the user that the plan was written and which features are now `PLANNED`.
