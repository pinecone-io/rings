# TODO

Implementation tasks, ready to build. The `/build` command picks up the next task from here.

---

## F-182: `rings init` — Workflow Scaffolding

**Spec:** `specs/cli/commands-and-flags.md` lines 232–293

**Summary:** `rings init [NAME]` scaffolds a new, immediately runnable `.rings.toml` file so users don't have to write boilerplate by hand.

### Task 1: CLI subcommand and argument parsing

**Files:** `src/cli.rs`, `src/main.rs`

**Steps:**
- [ ] Add `Init(InitArgs)` variant to `Command` enum in `src/cli.rs`
- [ ] Define `InitArgs` struct:
  - `name: Option<String>` — positional, defaults to `"workflow"`
  - `--force` (`bool`) — overwrite existing file
  - (global `--output-format` already exists on `Cli`)
- [ ] Add `Command::Init(args) => cmd_init(args, cli.output_format)` match arm in `main.rs`
- [ ] Stub `cmd_init` that just prints the resolved path and returns `Ok(())`

**Tests:**
- [ ] `rings init` parses with no args (name defaults to None)
- [ ] `rings init my-task` parses name as `Some("my-task")`
- [ ] `rings init --force` parses force flag

---

### Task 2: Path resolution and validation

**Files:** `src/main.rs` (or new `src/init.rs`)

**Steps:**
- [ ] Implement `resolve_init_path(name: Option<&str>) -> Result<PathBuf>`:
  - Default name: `"workflow"` → `./workflow.rings.toml`
  - Custom name: `"my-task"` → `./my-task.rings.toml`
  - Relative path: `"workflows/my-task"` → `./workflows/my-task.rings.toml`
  - Appends `.rings.toml` suffix if not already present
  - Rejects paths containing `..` components (exit code 2)
- [ ] Check if target file exists: if yes and `--force` not set, print error and exit 2
- [ ] If path has directory components (e.g. `workflows/`), verify parent dir exists (don't create it — exit 2 if missing)

**Tests:**
- [ ] Default name resolves to `workflow.rings.toml`
- [ ] Custom name appends `.rings.toml`
- [ ] Name already ending in `.rings.toml` is not double-suffixed
- [ ] Path with `..` is rejected with exit code 2
- [ ] Existing file without `--force` exits 2
- [ ] Existing file with `--force` succeeds

---

### Task 3: Template content and atomic write

**Files:** `src/main.rs` (or `src/init.rs`)

**Steps:**
- [ ] Define the scaffold template as a const string. Must include:
  - `[workflow]` with `completion_signal = "TASK_COMPLETE"`, `context_dir = "."`, `max_cycles = 10`, `completion_signal_mode = "line"`, `budget_cap_usd = 5.00`
  - One `[[phases]]` block named `"builder"` with `prompt_text` containing:
    - A useful starter prompt
    - The completion signal string embedded in the prompt text (so F-151 check passes)
    - A comment block listing all template variables: `{{phase_name}}`, `{{cycle}}`, `{{max_cycles}}`, `{{iteration}}`, `{{run}}`, `{{cost_so_far_usd}}`
- [ ] Write atomically: write to `<path>.tmp`, then `std::fs::rename` to final path
- [ ] On success, human mode: print `Created <path>` to stderr, then print a hint: `Run it with:  rings run <path>`
- [ ] On success, JSONL mode: emit `{"event":"init_complete","path":"<absolute_path>"}` to stdout

**Tests:**
- [ ] Scaffolded file parses as a valid `Workflow` via `Workflow::from_str`
- [ ] Scaffolded file passes `rings run --dry-run` (completion signal found in prompt)
- [ ] `budget_cap_usd` is present (F-116 no-cap warning won't fire)
- [ ] Template variables comment is present in prompt_text
- [ ] Atomic write: `.tmp` file does not remain after success
- [ ] JSONL output is valid JSON with correct `event` and `path` fields

---

### Task 4: Update feature inventory

**Steps:**
- [ ] Change F-182 status from `PRIORITIZED` to `COMPLETE` in `specs/feature_inventory.md`
- [ ] Update `REVIEW.md` with any decisions or open questions

---
