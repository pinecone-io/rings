Elect the next batch of high-priority features from the backlog into the priority queue.

## Steps

### 1. Find unblocked candidates

Read `specs/feature_inventory.md` in full.

A feature is an **unblocked candidate** if:
- Its status is `BACKLOG`, AND
- Every feature listed in its Summary as `requires F-XXX` has status `COMPLETE`, `PLANNED`, or `PRIORITIZED`

List all unblocked candidates. If there are none, tell the user the backlog is fully prioritized and stop.

### 2. Elect the next batch

Select up to 5 features to prioritize, ranked by these criteria in order:

1. **Dependency-unlocking power** — features that unblock the most other BACKLOG features come first
2. **User impact** — how much does this benefit real users (DevOps engineers, developers dogfooding rings)
3. **Implementation coherence** — prefer features that share a spec file or implementation surface with recently PLANNED/PRIORITIZED features
4. **Risk-adjusted effort** — simpler, well-specified features before complex or underspecified ones

Show the elected features to the user with brief rationale for each. Wait for confirmation before writing.

### 3. Write results (after user confirms)

Read `rings/plan-create/queue/PRIORITIZED_FEATURES.md` to find the current highest priority number N (look for the last `### Priority N:` heading). If empty, start at N = 0.

Append each elected feature in priority order:

```markdown
### Priority N+1: F-XXX — Feature Name

- **Summary:** (copy the one-line summary from feature_inventory.md)
- **Spec:** (copy the spec link from feature_inventory.md)
- **Unblocks:** (list any BACKLOG features that list this one as a prerequisite, or "none")

---
```

Then update `specs/feature_inventory.md`: change each elected feature's status from `BACKLOG` to `PRIORITIZED`. Change only the status cell.

Confirm to the user what was added and what the new priority numbers are.
