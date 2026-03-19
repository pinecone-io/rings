# rings

**Iterative AI workflows, without the shell scripts.**

rings runs Claude Code in a loop — cycling through your defined phases until work is done. You get cost tracking, safe cancellation, and resumable runs out of the box.

```
⠹  Cycle 4/20  │  builder  2/3  │  $0.34 total  │  1.2k in · 340 out  │  01:12
```

---

## Install

**Requires:** [Claude Code](https://claude.ai/code) installed and on your PATH.

```bash
# If you have the GitHub CLI authenticated:
curl -fsSL https://raw.githubusercontent.com/pinecone-io/rings/main/install.sh | bash

# Or with a token:
GITHUB_TOKEN=ghp_... curl -fsSL https://raw.githubusercontent.com/pinecone-io/rings/main/install.sh | bash

# From source:
cargo install --git https://github.com/pinecone-io/rings.git
```

Auto-detects your platform (Linux/macOS, x86_64/aarch64), verifies the SHA256 checksum, and installs to `/usr/local/bin/rings`. Pass a path to install elsewhere: `... | bash -s -- ~/.local/bin/rings`

Update to the latest version:

```bash
rings update
```

---

## Quickstart

Scaffold a new workflow:

```bash
rings init my-task
```

This creates `my-task.rings.toml` — a complete, runnable workflow that picks up tasks from `TODO.md`. Edit the prompt to fit your project, then:

```bash
rings run --dry-run my-task.rings.toml   # preview the execution plan
rings run my-task.rings.toml             # run it
```

### What the scaffold looks like

`rings init` generates a workflow pre-configured with:
- An `[executor]` section showing how to set the model (`--model claude-sonnet-4-6`)
- A prompt that reads `TODO.md`, finds the next unchecked task, does the work, and marks it done
- A completion signal that fires when all tasks are complete
- A budget cap so you don't accidentally run up costs

You'll want to customize the prompt's **Context** section to point at your project's important files (architecture docs, specs, contributing guide, etc.).

---

## How it works

You write a workflow file. rings runs it in cycles.

```
Cycle 1:  builder → builder → builder → reviewer
Cycle 2:  builder → builder → builder → reviewer
...
Cycle N:  builder prints APPROVED → rings exits
```

Each cycle executes your phases in order. Each phase invokes Claude Code with your prompt, in your working directory. When Claude prints your completion signal, the workflow exits. If it doesn't within `max_cycles`, rings saves state and tells you how to resume.

---

## Builder + reviewer pattern

The most common pattern: a builder writes code, a reviewer critiques it, repeat.

```toml
[workflow]
completion_signal = "APPROVED"
context_dir = "./src"
max_cycles = 20
completion_signal_phases = ["reviewer"]  # only reviewer can approve

[executor]
binary = "claude"
args = ["--dangerously-skip-permissions", "--output-format", "json", "--model", "claude-sonnet-4-6", "-p", "-"]

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

## Cost tracking

rings parses Claude Code's cost output after every run and keeps a running total. The status line shows cumulative cost and token counts updating in real time:

```
⠹  Cycle 3/10  │  builder  2/3  │  $1.47 total  │  18.2k in · 4.1k out  │  02:34
```

At completion, you get a per-phase breakdown:

```
✓  Completed — cycle 2, run 12 (builder)

   Duration    8m 14s
   Total cost  $1.10  (12 runs)
   Tokens      18,204 input · 4,102 output

   builder    ████████████████████  $0.89  (10 runs)
   reviewer   █████                 $0.21  ( 2 runs)

   Budget     ████████████░░░░░░░░  $1.10 / $5.00  (22%)
```

Set a budget cap to protect yourself:

```toml
[workflow]
budget_cap_usd = 5.00   # stop and save state if spend exceeds $5
```

Or at runtime: `rings run --budget-cap 5.00 task.rings.toml`

---

## Cancellation and resume

Press `Ctrl+C` at any point. rings saves where it was and prints the resume command:

```
✗  Interrupted

   Run ID      run_20240315_143022_a1b2c3
   Progress    cycle 3, builder 2/3 (23 runs)
   Cost        $2.14

   builder    ██████████████████    $1.71  (18 runs)
   reviewer   █████                 $0.43  ( 5 runs)

   To resume:
     rings resume run_20240315_143022_a1b2c3
```

No work is lost. Execution picks up from the next unstarted run.

---

## Commands

```bash
rings init [name]                    # scaffold a new workflow
rings run <workflow.toml>            # start a workflow
rings resume <run-id>                # resume an interrupted run
rings list                           # recent runs with status and cost
rings update                         # update rings to the latest version
```

Common flags:

```bash
rings run --dry-run workflow.toml    # preview execution plan
rings run --budget-cap 5 workflow.toml
rings run --max-cycles 5 workflow.toml
rings run --verbose workflow.toml    # stream Claude's output live
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

[executor]
binary = "claude"
# Change --model to use a different model
args = ["--dangerously-skip-permissions", "--output-format", "json", "--model", "claude-sonnet-4-6", "-p", "-"]

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

Declare what each phase reads and writes for early mismatch detection:

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

rings warns at startup if `consumes` files don't exist and warns after each run if `produces` patterns weren't touched.

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
