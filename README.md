# rings

**Iterative AI workflows, without the shell scripts.**

rings runs Claude Code in a loop — cycling through your defined phases until work is done. You get cost tracking, safe cancellation, and resumable runs out of the box.

```
⠹  Cycle 4/20  │  builder  2/3  │  $0.34 total  │  1.2k in · 340 out  │  01:12
```

---

## Install

**One-liner (macOS and Linux):**

```bash
curl -fsSL https://raw.githubusercontent.com/pinecone-io/rings/main/install.sh | bash
```

This auto-detects your platform, downloads the right binary, verifies the SHA256 checksum, and installs to `/usr/local/bin/rings`.

To install somewhere else: `curl -fsSL ... | bash -s -- ~/.local/bin/rings`

**With Rust installed:**

```bash
cargo install --git https://github.com/pinecone-io/rings.git
```

Verify:

```bash
rings --version
```

**Requires:** [Claude Code](https://claude.ai/code) installed and on your PATH.

---

## How it works

You write a workflow file. rings runs it in cycles.

Each cycle executes your phases in order. Each phase invokes Claude Code with your prompt, in your working directory. When Claude prints your completion signal, the workflow exits. If it doesn't within `max_cycles`, rings exits cleanly and tells you how to resume.

That's it.

---

## Quickstart

### The simplest workflow

Create `task.rings.toml`:

```toml
[workflow]
completion_signal = "DONE"
context_dir = "."
max_cycles = 10

[[phases]]
name = "builder"
prompt_text = """
Look at the code in this directory. Make it better.
Fix bugs, add tests, improve clarity.
When you're satisfied with the quality, print exactly: DONE
"""
```

Preview it first:

```bash
rings run --dry-run task.rings.toml
```

Run it:

```bash
rings run task.rings.toml
```

### Builder + reviewer loop

The most common pattern: a builder writes, a reviewer critiques, repeat.

```toml
[workflow]
completion_signal = "APPROVED"
context_dir = "./src"
max_cycles = 20
completion_signal_phases = ["reviewer"]  # only reviewer can approve

[[phases]]
name = "builder"
runs_per_cycle = 3
prompt_text = """
You are implementing a feature in ./src. Read TASK.md for requirements.
Read REVIEW_NOTES.md if it exists — it contains feedback from the last review.
Make progress. Write tests. When you think you're done, say so in your code comments.
"""

[[phases]]
name = "reviewer"
prompt_text = """
Review the code in ./src against the requirements in TASK.md.
Is it correct? Well-tested? Clean?
If it needs more work, write specific feedback to REVIEW_NOTES.md.
If it's ready to ship, print exactly: APPROVED
"""
```

This runs: builder, builder, builder, reviewer — then repeats. The reviewer controls completion; the builder can't short-circuit it.

### Prompt patterns

| Pattern | What it means | Config |
|---------|--------------|--------|
| `ABABAB` | Strict alternation | Both phases: `runs_per_cycle = 1` |
| `AAABAAAB` | Builder-heavy with periodic review | builder: `runs_per_cycle = 3` |
| `AAAAAABBBB` | Bulk work then bulk review | builder: `runs_per_cycle = 6`, reviewer: `runs_per_cycle = 4`, `max_cycles = 1` |
| Single phase | No reviewer, just iterate | One `[[phases]]` block |

---

## Testing a new prompt

When developing a prompt, use `--step` to pause after every run and inspect what happened before continuing:

```bash
rings run --step task.rings.toml
```

After each run, rings pauses and shows a summary:

```
─────────────────────────────────────────────────────
  Run 3  |  cycle 1  |  builder  (iteration 3/3)
  Cost:   $0.031  (1,204 input, 312 output tokens)
  Files:  2 modified  →  src/main.rs, src/engine.rs
  Signal: not detected

  [Enter] continue   [s] skip to next cycle   [v] view output   [q] quit
─────────────────────────────────────────────────────
```

Press `v` to read the full Claude Code output for that run. Press `q` to quit and save state. Press Enter to continue.

Use `--step-cycles` to pause only at cycle boundaries instead of after every run — less granular but less interruptive when a phase runs 3+ times per cycle.

---

## Cost tracking

rings parses Claude Code's cost output after every run and keeps a running total.

```
Cost Summary
─────────────────────────────────────────────────
Phase       Runs   Input Tok   Output Tok   Cost
─────────────────────────────────────────────────
builder       30    245,123      89,432     $3.12
reviewer      10     82,341      21,089     $1.11
─────────────────────────────────────────────────
TOTAL         40    327,464     110,521     $4.23
```

Set a budget cap to protect yourself on exploratory runs:

```toml
[workflow]
budget_cap_usd = 5.00   # stop and save state if spend exceeds $5
```

Or pass it at runtime:

```bash
rings run --budget-cap 5.00 task.rings.toml
```

---

## Cancellation and resume

Press `Ctrl+C` at any point. rings saves where it was and prints the resume command:

```
Interrupted.

Run ID:    run_20240315_143022_a1b2c3
Progress:  cycle 3, phase builder, run 2/3 (23 runs completed)

Cost so far: $2.14
  builder  (18 runs)  $1.71
  reviewer  (5 runs)  $0.43

To resume this run:
  rings resume run_20240315_143022_a1b2c3
```

Resume later:

```bash
rings resume run_20240315_143022_a1b2c3
```

No work is lost. Execution picks up from the next unstarted run.

---

## Inspecting runs

```bash
rings list                           # recent runs with status and cost
rings show <run-id>                  # one-screen summary
rings inspect <run-id>               # detailed view
rings inspect <run-id> --show costs  # per-phase cost breakdown
rings lineage <run-id>               # full ancestry chain across resumed sessions
```

---

## Workflow reference

### Common settings

```toml
[workflow]
completion_signal = "TASK_COMPLETE"     # required: string Claude prints when done
context_dir = "./src"                   # required: directory Claude Code works in
max_cycles = 20                         # optional: stop after N cycles (default: unlimited)
completion_signal_mode = "line"         # "substring" (default), "line", or "regex"
completion_signal_phases = ["reviewer"] # optional: only these phases can trigger completion
budget_cap_usd = 10.00                  # optional: stop if cost exceeds this amount
delay_between_runs = 2                  # optional: seconds between runs (default: 0)

[[phases]]
name = "builder"
prompt = "./prompts/builder.md"         # file reference
runs_per_cycle = 3

[[phases]]
name = "reviewer"
prompt_text = """                       # or inline text
Review the work. If done, print TASK_COMPLETE.
"""
```

### Prompt sources

Each phase uses either a file path or inline text — not both.

```toml
# File reference — good for long prompts or shared prompts
prompt = "./prompts/builder.md"

# Inline text — good for simple prompts or self-contained workflow files
prompt_text = """
Your prompt here.
Print TASK_COMPLETE when done.
"""
```

### Phase contracts (optional)

Declare what each phase reads and writes for lineage tracking and early mismatch detection:

```toml
[[phases]]
name = "builder"
prompt = "./prompts/build.md"
consumes = ["TASK.md", "REVIEW_NOTES.md"]
produces = ["src/**/*.rs", "tests/**/*.rs"]

[[phases]]
name = "reviewer"
prompt = "./prompts/review.md"
consumes = ["src/**/*.rs"]
produces = ["REVIEW_NOTES.md"]
```

rings warns at startup if `consumes` files don't exist and aren't mentioned in the prompt, and warns after each run if `produces` patterns weren't touched.

---

## Automation and CI

Use `--output-format jsonl` for structured output suitable for pipelines:

```bash
rings run --output-format jsonl workflow.toml | jq 'select(.event == "run_end")'
```

All structured events go to stdout; errors go to stderr. Interactive prompts are skipped in non-TTY contexts. Exit codes are documented and stable.

---

## `.gitignore`

rings stores run data in `~/.local/share/rings/runs/` by default — nothing to commit. If you configure a custom `output_dir` inside your repo:

```
# .gitignore
rings-output/
```

Workflow TOML files are designed to be committed — they contain no machine-specific state.

---

## Acknowledgments

The iterative AI prompting pattern at the core of rings — running a task many times with fresh context, accumulating progress through on-disk state — was discovered and popularized by [Geoffrey Huntley](https://ghuntley.com/loop/), who calls it the Ralph loop. rings automates that pattern.

---

## License

Apache 2.0 — see [LICENSE](LICENSE).
