Read rough ideas and evaluate them against the existing specs, then produce concrete spec proposals or new feature entries.

## Steps

1. Look for an `IDEAS.md` file at the project root. If it doesn't exist, tell the user to create one with their rough notes and stop.
2. Read `IDEAS.md` in full.
3. Read `specs/feature_inventory.md` to understand what is already specified.
4. Read `specs/index.md` and any spec files that are directly relevant to the ideas in `IDEAS.md`.
5. For each idea, evaluate:
   - **Already covered** — the idea is essentially described by an existing spec; note the feature number(s) and explain how.
   - **Extension of existing spec** — the idea adds nuance or a new option to a feature already in the inventory; propose a spec amendment.
   - **New feature** — the idea is genuinely not covered; draft a new feature entry suitable for adding to `specs/feature_inventory.md` and the appropriate spec file.
   - **Out of scope / conflict** — the idea conflicts with a design principle in `specs/overview.md` or `specs/mvp.md`; explain why and suggest an alternative if one exists.
6. For each **new feature** or **extension**, produce:
   - A proposed feature number (continuing from the highest existing F-NNN)
   - A one-line user-perspective summary in the style of the inventory
   - The spec file it belongs in
   - A short prose description (3–6 sentences) of the behavior, suitable for dropping into that spec file
   - Any dependency on existing features (cite F-NNN)
7. Summarize all proposals in a single output block the user can review before anything is written to disk. Do not modify any spec files until the user approves.
