# Cancellation and Resume

## Cancellation (Ctrl+C / SIGINT / SIGTERM)

### Signal Handling

rings installs handlers for both SIGINT and SIGTERM at startup. SIGTERM is handled identically to SIGINT — it triggers a graceful shutdown with state save. This allows process managers (systemd, supervisord, container runtimes) to cleanly stop rings.

When Ctrl+C is pressed (SIGINT) or SIGTERM is received:

1. **Set a cancellation flag** in the execution engine (checked after the current subprocess completes or on the next iteration boundary).
2. **Do not immediately kill** the running `claude` subprocess — first attempt a graceful shutdown.
3. **Send SIGTERM** to the `claude` subprocess.
4. **Wait up to 5 seconds** for it to exit cleanly.
5. **Send SIGKILL** if it does not exit within 5 seconds.
6. **Capture partial output** from the subprocess (everything captured so far).
7. **Scan partial output** for `claude resume <uuid>` lines.
8. **Save state** (see State Persistence below).
9. **Print cancellation summary** (see runtime-output.md).
10. **Exit with code 130** (standard convention for SIGINT exits).

### Why propagate to the executor subprocess

If the executor is mid-session when rings is canceled, it may have active state that can be resumed. By capturing any resume commands from its output (via the configured `resume_pattern`), the user is not left with orphaned, unresumable sessions. The printed resume commands let the user manually continue that specific session if desired.

### Double Ctrl+C

If the user presses Ctrl+C a second time while rings is waiting for the subprocess to exit:
- Send SIGKILL immediately (skip the remaining wait).
- Proceed to state save and exit.

## State Persistence

### State File

After every completed run, rings atomically writes `state.json` to `output_dir/<run-id>/state.json`.

```json
{
  "schema_version": 1,
  "run_id": "run_20240315_143022_a1b2c3",
  "workflow_file": "/abs/path/to/workflow.rings.toml",
  "last_completed_run": 7,
  "last_completed_cycle": 2,
  "last_completed_phase_index": 0,
  "last_completed_iteration": 3,
  "total_runs_completed": 7,
  "cumulative_cost_usd": 1.42,
  "claude_resume_commands": [
    "claude resume abc-123-def"
  ],
  "canceled_at": null
}
```

On cancellation, `canceled_at` is set to an ISO8601 timestamp.

The file is written atomically (write to a temp file, then `rename`) to prevent corruption if rings is killed mid-write.

### What "completed" means

A run is considered "completed" only after:
1. The `claude` subprocess has exited.
2. Its output has been fully captured.
3. The audit log has been written.
4. Cost has been recorded.

A run that was in-progress when rings was canceled is NOT marked as completed. Resume starts from the next run.

## Resuming a Run

### Command

```bash
rings resume run_20240315_143022_a1b2c3
```

### Process

1. rings searches for `state.json` in the known output directories (XDG data dir, then current dir).
2. Loads `workflow_file` path from `run.toml`.
3. Re-validates the workflow file (it must still exist and be parseable).
4. Computes the next run to execute based on `last_completed_run`.
5. Continues execution, writing to the same output directory.
6. Accumulates costs on top of previously recorded costs.

### State on Resume

On resume, rings prints:

```
Resuming run_20240315_143022_a1b2c3
Workflow:  my-task.rings.toml
Resuming from: cycle 3, builder, iteration 2/3
Previous cost: $2.14

Continuing...
```

### What Cannot Be Resumed

- A run that completed successfully (signal detected) cannot be resumed.
- A run where the workflow file has been deleted or moved cannot be resumed. rings prints a clear error showing the expected absolute path, e.g.:
  ```
  Error: Cannot resume run_20240315_143022_a1b2c3
    Workflow file not found: /home/alice/projects/my-task/task.rings.toml
    The workflow file may have been moved or deleted since the run was canceled.
    Options:
      - Restore the file to the expected path
      - Start a fresh run: rings run <new-path> --parent-run run_20240315_143022_a1b2c3
  ```
- A run where the state file is corrupted: rings prints the parse error and suggests running fresh.

### Cross-Machine Resume

Resume is not supported across machines. `state.json` stores the workflow file path as an absolute path on the machine where the run was started. A colleague who has the run ID but not access to the original machine's filesystem cannot resume the run — rings will fail with "workflow file not found" at the path from the other machine.

To hand off work to a colleague, use `--parent-run` to start a fresh run that records the ancestry link:

```bash
# Colleague starts fresh but preserves the work history chain
rings run task.rings.toml --parent-run run_20240315_143022_a1b2c3
```

If the output directory (`~/.local/share/rings/runs/run_...`) is on shared storage (NFS, cloud sync), resume will work cross-machine provided the workflow file is at the same absolute path on both machines.

## Run Listing for Resume Discovery

`rings list` shows all known runs with their status:

```
RUN ID                          DATE              STATUS     CYCLES  COST
run_20240315_143022_a1b2c3     2024-03-15 14:30  canceled    2/50   $2.14
run_20240314_091500_b3c4d5     2024-03-14 09:15  completed   7/50   $8.93
```

The run ID from this list can be passed directly to `rings resume`.

## Ctrl+C During a Delay

If the user presses Ctrl+C while rings is counting down a `delay_between_runs` or `delay_between_cycles` delay, rings triggers the normal cancellation flow immediately: the delay countdown is interrupted, the next run is NOT started, state is saved, and rings exits with code 130. This behavior is identical to Ctrl+C at any other point in execution.

## Resuming After Workflow File Changes

rings re-validates the workflow file when resuming. If the workflow file has been modified since the run was canceled:

- **Structural changes** (adding or removing phases, changing phase order, changing `name` fields): not supported on resume. rings exits with an error explaining that the phase structure cannot be reconciled with saved state. Use `--parent-run` to start fresh with the new structure and preserve the ancestry link.
- **Non-structural changes** (prompt text, `max_cycles`, `runs_per_cycle`, delay settings, budget cap): allowed. rings uses the new values going forward from the resume point and emits a warning: `"Workflow file has changed since this run was created. Non-structural changes will take effect from the resume point."`
- **`completion_signal` changes**: allowed but a warning is emitted, since the run's history was evaluated against a different signal.
- **Workflow file moved or deleted**: resume fails with a clear error showing the expected absolute path (see What Cannot Be Resumed above).

## Concurrent Run Protection

To prevent data corruption when two rings processes target the same `context_dir`, rings uses a lock file.

### Lock file behavior

When rings starts a run, it creates `<context_dir>/.rings.lock` containing the run ID and PID of the rings process. On exit (normal, canceled, or error), the lock file is removed.

### Startup lock check

If `.rings.lock` exists when rings tries to start:

1. rings reads the PID from the lock file.
2. If the PID is still running: rings exits with code 2:
   ```
   Error: Another rings run (run_ID, PID=X) is already using context_dir.
   Wait for it to finish or use --force-lock to override.
   ```
3. If the PID is no longer running (stale lock from a previously killed process): rings removes the stale lock, emits a warning, and proceeds:
   ```
   Warning: Removed stale lock file from previous run run_ID (PID=X no longer running).
   ```

### `--force-lock`

`--force-lock` skips the lock check entirely and proceeds regardless of whether a lock file exists. Useful in CI environments where stale locks from killed jobs are common. Use with caution — running two rings processes concurrently on the same `context_dir` without coordination can corrupt shared files.

## State Recovery on Corruption

If `state.json` is corrupted (unreadable, invalid JSON, or truncated), rings attempts graceful recovery before failing:

1. Scan `costs.jsonl` to determine the highest completed run number.
2. If readable with at least one entry, reconstruct minimal state: `last_completed_run` = max run number in `costs.jsonl`.
3. Resume proceeds from the next run after the recovered position.
4. A warning is emitted:
   ```
   Warning: state.json was unreadable; state reconstructed from costs.jsonl.
   Recovered to run N. If this is incorrect, start a new run with --parent-run to preserve ancestry.
   ```
5. If `costs.jsonl` is also unreadable, recovery is not possible. rings exits with code 2 and prints the absolute paths to both files for manual inspection.

## Claude Code Permissions Note

rings invokes Claude Code with `--dangerously-skip-permissions`. This flag disables Claude Code's interactive permission prompts, which would otherwise pause execution and wait for user input — making unattended runs impossible. It does **not** grant Claude Code additional OS-level permissions beyond those of the rings process itself. Claude Code can read and modify any file accessible to the running rings process within `context_dir`. Users should be aware of this when choosing what to include in `context_dir`.
