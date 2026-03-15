# CLI — Section Index

← [specs/index.md](../index.md)

All user-facing commands, flags, exit codes, and tooling for shell integration and binary distribution.

## Files

| File | Contents |
|------|----------|
| [commands-and-flags.md](commands-and-flags.md) | Full reference: `run`, `resume`, `list`, `cleanup`, `show`, `inspect`, `lineage` |
| [exit-codes.md](exit-codes.md) | Exit code table for all commands |
| [inspect-command.md](inspect-command.md) | `rings inspect` views: summary, cycles, files-changed, data-flow, costs, claude-output |
| [completion-and-manpage.md](completion-and-manpage.md) | Shell completions (bash/zsh/fish) and man page generation |
| [distribution.md](distribution.md) | Binary build config, static linking, platform targets |

## Related

- [State → configuration.md](../state/configuration.md) — config file and environment variable overrides that affect CLI behavior
- [Observability → audit-logs.md](../observability/audit-logs.md) — output directory structure that `rings run` writes and `rings inspect` reads
