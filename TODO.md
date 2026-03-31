# TODO

Implementation tasks, ready to build. The `/build` command picks up the next task from here.

---

## CI: Transition Release Pipeline from Push-Triggered to Cron-Triggered

**Ref:** `.github/workflows/ci.yml`

**Summary:** Every push to `main` currently triggers a full build+release pipeline, creating a version-bump commit (`chore: bump version to vX.Y.Z [skip ci]`) on every code push. This pollutes the git history with mechanical commits. Instead, split CI into two workflows: a push-triggered check-only workflow and a cron-triggered release workflow that wakes up hourly, compares the latest release tag to `HEAD`, and only bumps/builds/releases if there are unreleased code changes.

### Task 5: Test the new workflow split — SKIPPED

**Steps:**
- [x] Push the two-file split to `main`
- [ ] Verify a push triggers only the `ci.yml` check job (no release)
- [ ] Trigger `release.yml` manually via `workflow_dispatch` and confirm it detects unreleased changes, bumps version, builds, and publishes
- [ ] Verify that if no new commits exist since the last `v*` tag, the cron/manual run exits early without bumping or building
- [ ] Verify that multiple code pushes between cron ticks result in a single version bump (not one per push)

**Note:** Remaining steps require manual GitHub Actions verification; skipped by automated builder.

---

## F-175: Cargo Install Support

**Spec:** `specs/cli/distribution.md`

**Summary:** Rust users can install rings with `cargo install rings` without needing pre-built binaries. Requires publishing to crates.io.

### Task 1: Prepare for crates.io publishing

**Files:** `Cargo.toml`

**Steps:**
- [x] Verify `Cargo.toml` has required crates.io fields: `description`, `license`, `repository`, `keywords`, `categories`
- [x] Verify `cargo package` succeeds without errors (all required files included)
- [x] Add `exclude` patterns to keep the crate size reasonable (exclude test fixtures, specs, etc.)
- [x] Test with `cargo install --path .` locally

**Tests:**
- [x] `cargo install --path .` builds and installs successfully
- [x] `cargo package` produces a valid crate
- [x] `just validate` clean

---

## F-177: Reproducible Builds

**Spec:** `specs/cli/distribution.md`

**Summary:** Pin the Rust toolchain and commit Cargo.lock so any developer can reproduce the exact same release binary.

### Task 1: Pin toolchain and verify reproducibility

**Files:** `rust-toolchain.toml`, `Cargo.lock`

**Steps:**
- [x] Verify `rust-toolchain.toml` exists and pins a specific Rust version
- [x] Verify `Cargo.lock` is committed to the repository (not gitignored)
- [x] Document the build command in README or CONTRIBUTING: `cargo build --release --locked`
- [x] If already in place, mark as COMPLETE

**Tests:**
- [x] `cargo build --release --locked` succeeds
- [x] `rust-toolchain.toml` specifies exact version
- [x] `Cargo.lock` is tracked in git
- [x] `just validate` clean

---

## F-199, F-200, F-201: Named Locks for Concurrent Workflows

**Spec:** `specs/state/cancellation-resume.md`, `specs/workflow/workflow-file-format.md`

**Summary:** Allow multiple rings workflows to run concurrently against the same `context_dir` by assigning each a distinct `lock_name`. The lock file becomes `.rings.lock.<name>` instead of `.rings.lock`. Workflows with different lock names never block each other.

**Open Decisions:**
- Use `Option<String>` on `Workflow` (not a `LockName` newtype) — validation happens once in `validate()`, value consumed as `&str` downstream
- `lock_name` is not included in `structural_fingerprint()` — changing it on resume is a non-structural change
- Include lock name in stale-lock warning message for clarity (record in REVIEW.md)

### Task 1: Add `lock_name` field to workflow parsing and validation (F-201)

**Files:** `src/workflow.rs`

**Steps:**
- [ ] Add `lock_name: Option<String>` with `#[serde(default)]` to `WorkflowConfig`
- [ ] Add `lock_name: Option<String>` to `Workflow`
- [ ] Add `WorkflowError::InvalidLockName(String)` variant with message: `invalid lock_name "<value>": must match [a-z0-9_-]+`
- [ ] In `Workflow::validate()`, validate `lock_name` if present: reject empty string, validate with byte-level check (`!name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')`)
- [ ] Propagate validated `lock_name` from `WorkflowConfig` to `Workflow`

**Tests:**
- [ ] Valid names accepted: `"planner"`, `"build-01"`, `"a"`, `"my_workflow_1"`, `"123"`
- [ ] Invalid names rejected: `""` (empty), `"Planner"` (uppercase), `"my lock"` (space), `"a.b"` (dot), `"a/b"` (slash), `"a!b"` (punctuation)
- [ ] Absent `lock_name` field → `Workflow.lock_name == None`
- [ ] Error message includes the offending value and the allowed pattern
- [ ] `just validate` clean

### Task 2: Extend `ContextLock::acquire` for named locks (F-199, F-200)

**Files:** `src/lock.rs`

**Steps:**
- [ ] Add `lock_name: Option<&str>` parameter to `ContextLock::acquire`
- [ ] Extract helper: `fn lock_file_path(context_dir: &Path, lock_name: Option<&str>) -> PathBuf` returning `.rings.lock` or `.rings.lock.<name>`
- [ ] Use the helper for path computation in `acquire` (the stored `ContextLock.path` already drives RAII `Drop`)
- [ ] Add `lock_name: Option<String>` field to `LockError::ActiveProcess`
- [ ] Update `Display` for `ActiveProcess` to branch: `None` → `"is already using"` / `Some(name)` → `"holds lock \"{name}\" on"`
- [ ] Update all construction sites of `LockError::ActiveProcess` to pass `lock_name`

**Tests:**
- [ ] `lock_name = Some("planner")` creates `.rings.lock.planner`, not `.rings.lock`
- [ ] `lock_name = None` still creates `.rings.lock` (regression guard)
- [ ] Two different names held simultaneously in the same `context_dir` — both succeed
- [ ] Same name conflicts with itself (returns `ActiveProcess`)
- [ ] Named lock does not conflict with unnamed lock (and vice versa)
- [ ] Stale named lock detected and removed
- [ ] Force-lock with a named lock overwrites the correct file
- [ ] Drop removes the correct named lock file
- [ ] Error message for named lock includes `holds lock "planner"`
- [ ] Error message for unnamed lock matches existing format (regression)
- [ ] `just validate` clean

### Task 3: Wire `lock_name` through call sites in `main.rs`

**Files:** `src/main.rs`

**Steps:**
- [ ] At `run` call site (~line 573): pass `workflow.lock_name.as_deref()` to `ContextLock::acquire`
- [ ] At `resume` call site (~line 1033): pass `workflow.lock_name.as_deref()` to `ContextLock::acquire`
- [ ] Update stale-lock warning messages at both sites to include lock name when present
- [ ] Update existing tests in `tests/stale_lock_detection.rs` to pass `None` as fourth arg

**Tests:**
- [ ] Existing stale lock tests still pass with updated signature
- [ ] `just validate` clean

---

---
