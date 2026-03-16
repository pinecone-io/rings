# Feature Prioritization Election

You are running one cycle of the rings feature prioritization election. Your job is to elect
the highest-value unblocked features from the backlog, mark them as PRIORITIZED, and append
them to the ordered work queue.

---

## Step 1: Find unblocked candidates

Read `specs/feature_inventory.md` in full.

A feature is an **unblocked candidate** if:
- Its status is `BACKLOG`, AND
- Every feature listed in its Summary as `requires F-XXX` has status `COMPLETE`, `PLANNED`,
  or `PRIORITIZED`

List all unblocked candidates. If there are **none**, print exactly:

```
ALL_FEATURES_PRIORITIZED
```

and stop. Otherwise continue to Step 2.

---

## Step 2: Elect up to 3 features

From the unblocked candidates, select the highest-priority features — up to 3, or all of them
if fewer than 3 are available.

Rank by these criteria (in order):

1. **Dependency-unlocking power**: How many other BACKLOG features list this one as a
   prerequisite? Features that unblock entire subtrees come first.

2. **User impact**: How much does this feature benefit real users of rings? Think about:
   - DevOps engineers running rings in CI/CD
   - Developers dogfooding rings to build rings itself
   - Users who need safe, observable, interruptible automation

3. **Implementation coherence**: Does this feature naturally pair with recently PLANNED or
   PRIORITIZED features (shared code surface, same spec file, adjacent step)?

4. **Risk-adjusted effort**: Simpler, well-specified features should generally precede complex
   or underspecified ones that carry higher implementation risk.

---

## Step 3: Update feature_inventory.md

For each elected feature, change its status from `BACKLOG` to `PRIORITIZED` in
`specs/feature_inventory.md`. Change only the status cell — leave all other columns untouched.

---

## Step 4: Append to rings/prioritize/activities/PRIORITIZED_FEATURES.md

Read `rings/prioritize/activities/PRIORITIZED_FEATURES.md` to find the current highest priority number N (look for the
last `### Priority N:` heading). If the file has no entries yet, start at N = 0.

Append each elected feature as a new entry, incrementing N for each:

```markdown
### Priority N+1: F-XXX — Feature Name

- **Summary:** (copy the one-line summary from feature_inventory.md)
- **Spec:** (copy the spec link from feature_inventory.md)
- **Unblocks:** (list any BACKLOG features that list this one as a prerequisite, or "none")

---
```

Append entries in priority order (highest-priority elected feature first).

---

## Step 5: Confirm

After updating both files, print a one-line summary:

```
Elected: F-XXX (Name), F-YYY (Name), F-ZZZ (Name)
```
