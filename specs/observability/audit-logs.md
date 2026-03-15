# Audit Logs and Observability

## Output Directory Structure

Each rings run creates the following directory structure:

```
<output_dir>/<run-id>/
  run.toml           # Run metadata (workflow path, start time, rings version)
  state.json         # Current execution state (for resume)
  costs.jsonl        # Per-run cost data (newline-delimited JSON)
  runs/
    001.log          # stdout+stderr from run 1
    002.log          # stdout+stderr from run 2
    ...
  summary.md         # Written on completion; human-readable summary
```

## Run ID Format

Run IDs are formatted as: `run_YYYYMMDD_HHMMSS_<6-char-hex>`

Example: `run_20240315_143022_a1b2c3`

The hex suffix is derived from a random UUID to avoid collisions if two runs start in the same second.

## run.toml

```toml
run_id = "run_20240315_143022_a1b2c3"
workflow_file = "/abs/path/to/my-task.rings.toml"
started_at = "2024-03-15T14:30:22Z"
rings_version = "0.1.0"
status = "running"  # running | completed | canceled | failed
```

`status` is updated on exit.

## Output Directory Structure (complete)

```
<output_dir>/<run-id>/
  run.toml               # Run metadata (includes ancestry: parent_run_id, continuation_of)
  state.json             # Execution state (for resume)
  costs.jsonl            # Per-run cost + file diff data (newline-delimited JSON)
  runs/
    001.log              # stdout+stderr from run 1
    002.log              # ...
  manifests/
    000-before.json.gz   # context_dir state before first run
    001-after.json.gz    # context_dir state after run 1
    002-after.json.gz    # ...
  snapshots/             # only present when snapshot_cycles = true
    cycle-000-before/    # full copy of context_dir before execution
    cycle-001-after/     # full copy of context_dir after cycle 1
    ...
  summary.md             # Written on completion; human-readable summary
```

## Output Directory Security

### Directory Permissions

rings creates the run output directory with mode `0700` (owner read/write/execute only). Individual files within it are created with mode `0600` (owner read/write only). This prevents other users on a shared system from reading audit logs, cost data, or captured Claude Code output.

If the output directory already exists with broader permissions (e.g., created by the user manually), rings does not downgrade permissions but does warn:

```
Warning: Output directory has broad permissions (mode 755). Consider: chmod 700 <path>
```

### Path Traversal Protection

rings validates the `output_dir` value to ensure it does not contain path traversal sequences (`..`). If validation fails, rings exits with code 2:

```
Error: output_dir contains path traversal ('..') which is not allowed.
```

Similarly, rings validates `context_dir` to ensure it is an absolute path or resolves to one within the user's current directory tree. Symlinks in `context_dir` are followed but the resolved path is validated.

## Run Log Files (`runs/NNN.log`)

Each log file contains:
- Header line: `# Run 001 | cycle=1 phase=builder iteration=1/3 started=2024-03-15T14:30:25Z`
- Full stdout+stderr from the `claude` invocation

Log files are written as they are captured — even a partially-captured log is better than no log. If rings is killed mid-run, the partial log is preserved.

> **Security note:** Run log files contain the full stdout/stderr of Claude Code subprocess invocations. This may include content from the context directory if Claude reads and outputs files during execution. Users should treat run logs as sensitive data. The run output directory is created with mode `0700` and individual files with mode `0600`, restricting access to the owner only.

## costs.jsonl (updated schema)

Each entry now includes file diff data alongside cost:

```jsonl
{"run":1,"phase":"builder","cycle":1,"iteration":1,"cost_usd":0.092,"input_tokens":1234,"output_tokens":567,
 "cost_confidence":"full","files_added":[],"files_modified":["src/main.rs","src/engine.rs"],"files_deleted":[],"files_changed":2}
```

`cost_confidence` values: `"full"` (all fields parsed), `"partial"` (cost present, tokens absent), `"low"` (uncertain match), `"none"` (parse failed). This enables queries like `jq 'select(.cost_confidence == "low")' costs.jsonl`.

## state.json

Written after each completed run:

```json
{
  "schema_version": 1,
  "run_id": "run_20240315_143022_a1b2c3",
  "last_completed_run": 7,
  "last_completed_cycle": 2,
  "last_completed_phase_index": 0,
  "last_completed_iteration": 3,
  "total_runs_completed": 7,
  "claude_resume_commands": [
    "claude resume abc-123-def",
    "claude resume xyz-456-uvw"
  ]
}
```

`claude_resume_commands` is populated from any `claude resume` lines found in run output. This preserves partial work from canceled Claude Code sessions.

## run.toml (updated schema)

```toml
run_id = "run_20240315_143022_a1b2c3"
workflow_file = "/abs/path/to/my-task.rings.toml"
started_at = "2024-03-15T14:30:22Z"
rings_version = "0.1.0"
status = "running"          # running | completed | canceled | failed
executor_version = "1.2.3"   # optional; captured from executor version string early in first run output; absent if not detected

# Present only when status = "failed":
# failure_reason = "quota"  # "quota" | "auth" | "timeout" | "unknown"

# Ancestry (see lineage/run-ancestry.md)
parent_run_id = "run_20240315_120000_p1q2r3"   # null if root run
continuation_of = "run_20240315_120000_p1q2r3"  # null if root run
ancestry_depth = 1                              # 0 for root runs
```

## summary.md

Written on workflow completion or clean cancellation:

```markdown
# rings Run Summary: run_20240315_143022_a1b2c3

**Status:** Completed (signal detected)
**Workflow:** my-task.rings.toml
**Duration:** 14m 32s
**Completed:** 2024-03-15T14:44:54Z

## Execution

| Cycle | Phase    | Runs |
|-------|----------|------|
| 1     | builder  | 3    |
| 1     | reviewer | 1    |
| 2     | builder  | 2    |

Completed on cycle 2, phase builder, iteration 2.

## Cost

| Phase    | Runs | Cost   |
|----------|------|--------|
| builder  | 5    | $0.89  |
| reviewer | 1    | $0.21  |
| **Total**| **6**| **$1.10** |

## Output Location

Audit logs: /home/user/.local/share/rings/runs/run_20240315_143022_a1b2c3/
```
