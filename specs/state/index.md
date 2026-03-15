# State — Section Index

← [specs/index.md](../index.md)

How rings persists and recovers execution state: configuration loading, safe cancellation and resume, and run ancestry tracking.

## Files

| File | Contents |
|------|----------|
| [configuration.md](configuration.md) | Config file locations (XDG), precedence rules, per-run flag overrides, security considerations |
| [cancellation-resume.md](cancellation-resume.md) | SIGINT/SIGTERM handling, lock files, state persistence, recovery sequence |
| [run-ancestry.md](run-ancestry.md) | `parent_run_id` chain linking resumed runs, OTel span links for distributed tracing |

## Related

- [Execution → engine.md](../execution/engine.md) — where state transitions are triggered
- [Observability → audit-logs.md](../observability/audit-logs.md) — the on-disk format that state is persisted to
- [CLI → commands-and-flags.md](../cli/commands-and-flags.md) — `rings resume` and `rings lineage` commands
