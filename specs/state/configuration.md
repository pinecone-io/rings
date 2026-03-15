# Configuration

## Design Principle

rings is intentionally stateless at the command level. All workflow behavior is specified either in the workflow TOML file or as CLI flags. The config file holds only user-level defaults — not workflow definitions.

This means a workflow file is fully self-contained and portable. Sharing a `.rings.toml` workflow file with a colleague gives them everything they need to run it.

## Config File Locations

rings searches for configuration in this priority order (first found wins):

1. `.rings-config.toml` in the current working directory (project-level override)
2. `$XDG_CONFIG_HOME/rings/config.toml` (typically `~/.config/rings/config.toml`)

If no config file is found, all defaults apply.

**Note:** These are the *rings configuration* files, not workflow files. Workflow files may be named anything (conventionally `<task>.rings.toml`) and are passed explicitly on the command line.

**Naming note:** The project-level config file is named `.rings-config.toml` (not `.rings.toml`) to avoid confusion with workflow files, which are often named `<task>.rings.toml` by convention. A plain `.rings.toml` in a directory is likely a workflow file, not a config file.

## Config File Schema

```toml
# ~/.config/rings/config.toml  (or .rings-config.toml in project root)

# Default directory for run output (audit logs, state, costs).
# Supports ~ expansion.
# Default: $XDG_DATA_HOME/rings/runs  (typically ~/.local/share/rings/runs)
# default_output_dir = "~/.local/share/rings/runs"

# Enable colored output. Set to false to always disable color.
# rings also respects the NO_COLOR environment variable.
# Default: true
# color = true

# Default for --no-completion-check flag.
# If true, the completion signal warning is suppressed globally.
# Default: false
# skip_completion_check = false

# Default for --strict-parsing flag.
# If true, Low or None confidence parse results halt execution (exit 2) instead
# of accumulating as warnings. Useful for monitoring pipelines where incomplete
# observability data is unacceptable.
# Default: false
# strict_parsing = false

# Default executor settings. Applied to all workflows that do not specify [executor].
# Per-workflow [executor] blocks override these entirely for that workflow.
# Useful for users who always use a non-default executor and want to avoid
# repeating the same [executor] block in every workflow file.
#
# [executor]
# binary = "claude"
# args = ["--dangerously-skip-permissions", "-p", "-"]
# cost_parser = "claude-code"
# error_profile = "claude-code"
```

All config values are optional. An empty config file is valid.

## XDG Directories

rings follows the XDG Base Directory Specification:

| Purpose | Default Path |
|---------|-------------|
| User config file | `~/.config/rings/config.toml` |
| Project config file | `.rings-config.toml` in CWD (optional) |
| Run output (data) | `~/.local/share/rings/runs/` |

These directories are created on first use if they do not exist.

## Per-Run Overrides

CLI flags override config file values for a single run:

| Config Key | CLI Override |
|-----------|-------------|
| `default_output_dir` | `--output-dir` |
| `color` | `--no-color` |
| `skip_completion_check` | `--no-completion-check` |

## Argument Precedence

When the same setting can be specified in multiple places, rings resolves conflicts using this priority order (highest to lowest):

```
1. CLI flag          (--max-cycles 5, --output-dir ./out, --no-color)
2. Workflow TOML     ([workflow] max_cycles, output_dir)
3. Project config    (.rings-config.toml in current working directory)
4. User config       (~/.config/rings/config.toml)
5. Built-in default  (documented per-setting default)
```

**Example:** If `~/.config/rings/config.toml` sets `default_output_dir = "~/rings-runs"`, but the workflow file sets `output_dir = "./local-output"`, and the user passes `--output-dir /tmp/run`, then `/tmp/run` wins.

**Which settings participate in precedence:**

| Setting | CLI flag | Env var | Workflow TOML | Config file | Default |
|---------|----------|---------|---------------|-------------|---------|
| output directory | `--output-dir` | `RINGS_OUTPUT_DIR` | `output_dir` | `default_output_dir` | XDG data dir |
| max cycles | `--max-cycles` | — | `max_cycles` | — | unlimited |
| color output | `--no-color` | `NO_COLOR` | — | `color` | true |
| completion check | `--no-completion-check` | — | — | `skip_completion_check` | false |
| strict parsing | `--strict-parsing` | — | — | `strict_parsing` | false |
| output format | `--output-format` | `RINGS_OUTPUT_FORMAT` | — | `output_format` | human |
| budget cap | `--budget-cap` | — | `budget_cap_usd` | — | none |

**Argument precedence (highest → lowest):** CLI flag > env var > workflow TOML > project config > user config > built-in default

Note: `context_dir` and `completion_signal` are workflow-only settings. They cannot be overridden by the config file, only by future per-run flags if added.

## No Secrets in Config

rings does not store API keys or credentials. Authentication is handled by the executor directly. API keys and credentials are set as environment variables by the user and passed through to the executor via the inherited environment.

## Security Considerations

### CWD Config File Trust

`.rings-config.toml` files in the current working directory are automatically loaded and can override any configuration, including `default_output_dir`, contract check settings, and budget caps. In shared directory environments (shared repos, `/tmp`, network mounts), an untrusted `.rings-config.toml` could silently alter rings's behavior.

rings emits an info-level message whenever a CWD-local config file is loaded:

```
Loading local config from ./.rings-config.toml
```

Users in sensitive environments should audit any `.rings-config.toml` files present in their working directory before running rings.

### Environment Variable Pass-Through

rings passes the current shell environment to the executor subprocess without filtering. Environment variables including API keys and other credentials are visible to the executor. Users should not rely on environment isolation — treat the executor subprocess as having the same environment access as the rings process itself.

### Sensitive Files in context_dir

When `context_dir` contains files matching common credential patterns (`.env`, `*.key`, `*.pem`, `.aws/credentials`, `.ssh/id_*`, `*_rsa`, `*.pfx`, `*.p12`), rings emits a startup advisory warning:

```
Warning: context_dir contains credential files that may be readable by the executor:
  .env, .aws/credentials
The executor has read access to all files in context_dir.
Suppress with --no-sensitive-files-check if intentional.
```

This warning can be suppressed with `--no-sensitive-files-check` when the user has intentionally included credential files and accepts the risk.
