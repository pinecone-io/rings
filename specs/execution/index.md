# Execution — Section Index

← [specs/index.md](../index.md)

How rings drives Claude Code runs: the core engine loop, subprocess invocation, prompt rendering, output parsing, rate limiting, and error handling.

## Files

| File | Contents |
|------|----------|
| [engine.md](engine.md) | Startup sequence, core run loop, resume sequence, advisory checks |
| [executor-integration.md](executor-integration.md) | Executor interface, default Claude Code config, per-phase overrides, env passthrough |
| [prompt-templating.md](prompt-templating.md) | Template variables (`{{phase_name}}`, `{{cycle}}`, `{{cost_so_far_usd}}`, etc.) |
| [completion-detection.md](completion-detection.md) | Completion signal matching modes: substring, line-anchored, regex |
| [output-parsing.md](output-parsing.md) | Cost extraction from `claude` output, confidence levels, strict mode, warning deduplication |
| [error-handling.md](error-handling.md) | Error classification (quota, auth, unknown), state persistence on error |
| [rate-limiting.md](rate-limiting.md) | Run/cycle delays, automatic quota backoff and retry |

## Related

- [Workflow → cycle-model.md](../workflow/cycle-model.md) — the sequencing model the engine implements
- [State → cancellation-resume.md](../state/cancellation-resume.md) — SIGINT/SIGTERM handling during active runs
- [Observability → runtime-output.md](../observability/runtime-output.md) — what the engine emits to the terminal during execution
