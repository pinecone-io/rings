Review the feature inventory and produce a detailed implementation plan for the next batch of features to work on.

## Steps

1. Read `specs/feature_inventory.md` to get the full feature list with current statuses.
2. Read `specs/mvp.md` to understand original scope priorities.
3. Read `specs/index.md` to orient yourself in the spec tree.
4. Identify all features that are `BACKLOG` and group them by their dependency relationships — features whose prerequisites are all `COMPLETE` are unblocked and eligible for the next batch.
5. For each candidate feature, read the relevant spec file linked in the inventory to understand the full requirements.
6. Propose a prioritized batch of 5–10 features to tackle next, chosen for:
   - All prerequisites already `COMPLETE`
   - High user value relative to implementation complexity
   - Logical grouping (features that share a spec file or implementation surface are good candidates to bundle)
7. For each selected feature, produce a detailed implementation plan:
   - Which source files to create or modify
   - Key data structures or types to add
   - Test cases required (unit and integration) per the testing rules in `CLAUDE.md`
   - Any spec ambiguities to note in `REVIEW.md`
8. Output the plan in a format ready to be saved as `PLAN.md` at the project root.
