[2026-03-16] output should be colorful and easy to visually parse
→ Already covered: F-125 (Human Output Mode), F-128 (Status Line Display), F-129 (Animated Spinner), F-130 (Phase Transition Lines), and the full Runtime Output section — all of which are COMPLETE and together provide colored, spinner-animated terminal output.

[2026-03-16] need some visual feedback about the phase in progress actually doing something. A timer or some dots or something.
→ Already covered: F-128 (Status Line Display) and F-129 (Animated Spinner) are both COMPLETE and together provide a live-updating status line with an animated spinner confirming rings is alive during long Claude invocations.

[2026-03-16] we should expose model selection through the toml config — each phase should be configurable separately; some tasks are much better suited to a cheap model than an expensive model
→ F-181: Per-Phase Model Selection via `executor.extra_args`

[2026-03-16] init command to scaffold a new workflow file
→ F-182: `rings init`
