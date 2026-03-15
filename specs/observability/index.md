# Observability — Section Index

← [specs/index.md](../index.md)

Everything rings emits: terminal output during runs, structured audit logs written to disk, cost tracking, file change snapshots, and optional OpenTelemetry integration.

## Files

| File | Contents |
|------|----------|
| [runtime-output.md](runtime-output.md) | Status line display, human vs JSONL output modes, step-through mode, verbose mode |
| [audit-logs.md](audit-logs.md) | Output directory layout: `run.toml`, `state.json`, `costs.jsonl`, `summary.md`, per-run logs |
| [cost-tracking.md](cost-tracking.md) | Cost data model, real-time display, per-phase summaries, budget caps and warnings |
| [file-lineage.md](file-lineage.md) | File manifest, cycle snapshots, mtime optimization, credential file protection |
| [opentelemetry.md](opentelemetry.md) | Opt-in OTel traces and metrics, span attributes, Prometheus/Grafana query examples |

## Related

- [CLI → inspect-command.md](../cli/inspect-command.md) — querying audit logs interactively after a run
- [State → run-ancestry.md](../state/run-ancestry.md) — ancestry links surfaced in OTel span context
- [Execution → output-parsing.md](../execution/output-parsing.md) — how cost figures are extracted from `claude` output before being recorded
