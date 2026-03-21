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

## F-173: macOS Universal Binary

**Spec:** `specs/cli/distribution.md`

**Summary:** On macOS, produce a single universal binary that runs natively on both Intel and Apple Silicon using `lipo` to combine x86_64 and aarch64 builds.

### Task 1: Add universal binary step to release workflow

**Files:** `.github/workflows/release.yml`

**Steps:**
- [x] After building both `x86_64-apple-darwin` and `aarch64-apple-darwin` targets, combine with `lipo -create -output rings-macos-universal rings-x86_64 rings-aarch64`
- [x] Upload the universal binary as a release asset alongside the per-arch binaries
- [x] Verify the universal binary runs on both architectures: `file rings-macos-universal` shows "Mach-O universal binary"

**Tests:**
- [x] Universal binary contains both x86_64 and arm64 slices
- [x] `just validate` clean

---

## F-174: Binary Size Optimization

**Spec:** `specs/cli/distribution.md`

**Summary:** Target < 5 MB binary size. Configure Cargo profile for release builds to minimize binary size.

### Task 1: Optimize release profile

**Files:** `Cargo.toml`

**Steps:**
- [ ] In `[profile.release]`, set `opt-level = "z"` (optimize for size) or `opt-level = "s"`
- [ ] Set `lto = true` for link-time optimization
- [ ] Set `codegen-units = 1` for better optimization (slower build, smaller binary)
- [ ] Set `strip = true` to strip debug symbols from release binary
- [ ] Measure binary size before and after: `ls -lh target/release/rings`
- [ ] If size is still > 5 MB, consider `panic = "abort"` to remove unwinding code

**Tests:**
- [ ] Release binary is < 5 MB
- [ ] Binary still passes all tests after optimization
- [ ] `just validate` clean

---

## F-175: Cargo Install Support

**Spec:** `specs/cli/distribution.md`

**Summary:** Rust users can install rings with `cargo install rings` without needing pre-built binaries. Requires publishing to crates.io.

### Task 1: Prepare for crates.io publishing

**Files:** `Cargo.toml`

**Steps:**
- [ ] Verify `Cargo.toml` has required crates.io fields: `description`, `license`, `repository`, `keywords`, `categories`
- [ ] Verify `cargo package` succeeds without errors (all required files included)
- [ ] Add `exclude` patterns to keep the crate size reasonable (exclude test fixtures, specs, etc.)
- [ ] Test with `cargo install --path .` locally

**Tests:**
- [ ] `cargo install --path .` builds and installs successfully
- [ ] `cargo package` produces a valid crate
- [ ] `just validate` clean

---

## F-177: Reproducible Builds

**Spec:** `specs/cli/distribution.md`

**Summary:** Pin the Rust toolchain and commit Cargo.lock so any developer can reproduce the exact same release binary.

### Task 1: Pin toolchain and verify reproducibility

**Files:** `rust-toolchain.toml`, `Cargo.lock`

**Steps:**
- [ ] Verify `rust-toolchain.toml` exists and pins a specific Rust version
- [ ] Verify `Cargo.lock` is committed to the repository (not gitignored)
- [ ] Document the build command in README or CONTRIBUTING: `cargo build --release --locked`
- [ ] If already in place, mark as COMPLETE

**Tests:**
- [ ] `cargo build --release --locked` succeeds
- [ ] `rust-toolchain.toml` specifies exact version
- [ ] `Cargo.lock` is tracked in git
- [ ] `just validate` clean

---

---
