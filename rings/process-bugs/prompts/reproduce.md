# Reproduce the Bug

You are the **reproduce** phase of the **bug-fixing** rings workflow.

Your job is to confirm the bug described in `bug-working.md` is reproducible before
any fix is attempted.

---

## Step 1: Load context

Read `bug-working.md` in full to understand the bug title, description, and severity.

---

## Step 2: Locate the relevant code

Find the code path most likely responsible for the described behavior. Read the relevant
files. Note the file paths and function names involved in `bug-working.md`.

---

## Step 3: Attempt reproduction

Try to trigger the bug. The preferred approach is a failing test — write one if a natural
test entry point exists. Otherwise, trace the code path manually and identify the exact
condition under which the incorrect behavior occurs.

---

## Step 4: Document results and decide next step

Under `## Reproduction Steps` in `bug-working.md`, record:
- How you attempted to reproduce it
- Whether reproduction succeeded
- The exact code path or test that demonstrates the failure (file:line if applicable)

**If reproduction succeeded:** leave any failing test in place — the fix phase will make it
pass. Print a one-line status:

```
Reproduced: "<title>" — succeeded
```

**If reproduction failed:** do not proceed to fix. Remove the bug entry from `## Open`
in `rings/process-bugs/queue/BUG_REPORT.md`. Append to
`rings/process-bugs/queue/NEEDS_INFO.md` (create the file if absent):

```
- [ ] **<title>**: <original description>
  → Could not reproduce: <one sentence on what was tried and why it may be environment-specific>
```

Delete `bug-working.md`. Then print exactly:

```
RINGS_CONTINUE
```

and stop.
