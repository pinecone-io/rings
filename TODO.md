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

## F-178: Shell Completions Behavior

**Spec:** `specs/cli/completion-and-manpage.md`

**Summary:** Tab-completion offers `.toml` files for workflow arguments, run IDs for resume/show/inspect arguments, and flag names everywhere. Requires clap_complete's custom completer support.

### Task 1: Add custom completers for arguments

**Files:** `src/cli.rs`, `src/main.rs`

**Steps:**
- [x] For `<WORKFLOW>` argument in `rings run`: add a custom completer that suggests `.rings.toml` and `.toml` files in the current directory
- [x] For `<RUN_ID>` arguments in `rings resume`, `rings show`, `rings inspect`, `rings lineage`: add a custom completer that lists run IDs from the output directory
- [x] Use `clap_complete::engine::ArgValueCompleter` or shell-specific completion scripts
- [x] Test with `rings completions zsh` and verify completions work in zsh

**Tests:**
- [x] Generated completion script contains workflow file completion logic
- [x] Generated completion script contains run ID completion logic
- [x] `just validate` clean

---

## F-162: OpenTelemetry Opt-In

**Spec:** `specs/observability/opentelemetry.md`

**Summary:** Add opt-in OpenTelemetry tracing, controlled by `RINGS_OTEL_ENABLED=1` environment variable. When enabled, rings exports traces to an OTLP-compatible collector. When disabled (default), no tracing overhead.

### Task 1: Add OTel initialization

**Files:** `src/otel.rs` (new), `src/lib.rs`, `src/engine.rs`, `Cargo.toml`

**Steps:**
- [ ] Add `opentelemetry`, `opentelemetry-otlp`, and `tracing-opentelemetry` to `Cargo.toml` as optional dependencies behind an `otel` feature flag
- [ ] Create `src/otel.rs` with `init_tracer() -> Result<Option<SdkTracerProvider>>`:
  1. Check `RINGS_OTEL_ENABLED` env var — if not set or "0", return `None` (no-op)
  2. Read `OTEL_EXPORTER_OTLP_ENDPOINT` for collector endpoint (F-170)
  3. Initialize OTLP exporter and tracer provider
  4. If init fails, print warning and continue with no-op tracer (F-169)
- [ ] Register `pub mod otel;` in `src/lib.rs`
- [ ] In engine startup: call `init_tracer()`, store the provider for shutdown at exit
- [ ] On exit: call `provider.shutdown()` to flush remaining spans

**Tests:**
- [ ] `RINGS_OTEL_ENABLED=0`: no tracer initialized, no overhead
- [ ] `RINGS_OTEL_ENABLED=1` with no endpoint: warning printed, continues with no-op
- [ ] Feature flag `otel` controls compilation of dependencies
- [ ] `just validate` clean (with and without `otel` feature)

---

## F-163/F-164/F-165: OTel Trace Structure and Span Attributes

**Spec:** `specs/observability/opentelemetry.md`

**Summary:** When OTel is enabled, emit one trace per workflow run with spans for each cycle and phase-run. Spans carry attributes like phase name, cost, file change counts. Failed runs mark spans as ERROR.

### Task 1: Add trace structure with cycle and run spans

**Files:** `src/otel.rs`, `src/engine.rs`

**Steps:**
- [ ] Create a root span for the entire workflow run with attributes: `run_id`, `workflow`, `max_cycles`
- [ ] Create child spans for each cycle: `rings.cycle` with `cycle_number` attribute
- [ ] Create child spans for each phase-run: `rings.run` with attributes: `phase_name`, `iteration`, `cost_usd`, `input_tokens`, `output_tokens`, `files_changed`
- [ ] On non-zero executor exit: set span status to ERROR with the error message
- [ ] On completion signal: add `rings.completion_signal` event to the triggering run span
- [ ] All span creation is no-op when OTel is disabled (behind the feature flag)

**Tests:**
- [ ] With OTel enabled: spans are created with correct parent-child hierarchy
- [ ] Span attributes contain expected values
- [ ] Failed run span has ERROR status
- [ ] OTel disabled: no spans created, no overhead
- [ ] `just validate` clean

---

## F-180: Man Page Generation

**Spec:** `specs/cli/completion-and-manpage.md`

**Summary:** Generate a man page from the CLI definition so users can read `man rings` for offline documentation.

### Task 1: Add man page generation

**Files:** `build.rs` (or `src/main.rs`), `Cargo.toml`

**Steps:**
- [ ] Add `clap_mangen` to `Cargo.toml` build-dependencies
- [ ] In `build.rs`: generate man page from the `Cli` struct using `clap_mangen::Man::new(cmd).render(&mut buf)`
- [ ] Write the generated man page to `target/man/rings.1` during build
- [ ] Add a `just man` recipe that copies the generated man page to a standard location
- [ ] Alternatively: add a `rings --generate-man` hidden flag that prints the man page to stdout

**Tests:**
- [ ] Generated man page is valid roff format
- [ ] Man page includes all subcommands and flags
- [ ] `just validate` clean

---

## F-171/F-172: Static Binary and Multi-Platform Release

**Spec:** `specs/cli/distribution.md`

**Summary:** Produce static binaries with no system library dependencies for x86_64 and aarch64 on Linux and macOS. This is primarily CI/build configuration.

### Task 1: Configure static linking in CI

**Files:** `.github/workflows/release.yml`, `Cargo.toml`

**Steps:**
- [ ] For Linux builds: use `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` targets for fully static binaries
- [ ] For macOS: standard builds are already effectively static (no dynamic deps beyond system frameworks)
- [ ] Add cross-compilation targets to the release workflow matrix
- [ ] Verify binary has no dynamic library dependencies: `ldd target/release/rings` shows "not a dynamic executable"
- [ ] Verify binary size is reasonable (< 10 MB target)

**Tests:**
- [ ] Linux musl binary runs without any shared libraries
- [ ] macOS binary runs on both Intel and Apple Silicon (if universal)
- [ ] `just validate` clean on each target

---

## F-176: SHA256 Checksums for Releases

**Spec:** `specs/cli/distribution.md`

**Summary:** Every release includes SHA256 checksums so users can verify binary integrity after download.

### Task 1: Add checksum generation to release workflow

**Files:** `.github/workflows/release.yml`

**Steps:**
- [ ] After building each platform binary, compute SHA256: `sha256sum rings-{target} > rings-{target}.sha256`
- [ ] Upload checksum files alongside binaries as release assets
- [ ] Include a combined `SHA256SUMS` file listing all binaries
- [ ] Document verification in README: `sha256sum -c rings-x86_64-unknown-linux-musl.sha256`

**Tests:**
- [ ] Each release asset has a corresponding `.sha256` file
- [ ] Checksum file content matches the actual binary hash
- [ ] `just validate` clean

---
