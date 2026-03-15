# Shell Completion and Man Pages

## Shell Completion

rings ships with completion scripts for bash, zsh, and fish. These are generated from the clap CLI definition using `clap_complete`.

### Installation

Completion scripts are installed automatically by the system package manager. For manual installation:

```bash
# bash — write to a dedicated file and source it (do not append to .bashrc)
mkdir -p ~/.local/share/bash-completion/completions
rings completions bash > ~/.local/share/bash-completion/completions/rings
# bash auto-sources files in ~/.local/share/bash-completion/completions/ on modern systems
# or add to /etc/bash_completion.d/rings for system-wide installation

# zsh (~/.zfunc/ must be in $fpath)
mkdir -p ~/.zfunc
rings completions zsh > ~/.zfunc/_rings

# fish (~/.config/fish/completions/)
rings completions fish > ~/.config/fish/completions/rings.fish
```

### Completion Behavior

- **`rings run <TAB>`**: completes `.toml` files in the current directory
- **`rings resume <TAB>`**: completes known run IDs from the output directory
- **`rings list`**: no arguments to complete
- **All flags**: complete with their long form descriptions

### Implementation Note

The `rings completions <SHELL>` subcommand generates and prints the completion script to stdout. It is a hidden subcommand (not shown in main help).

## Man Pages

rings ships man pages generated via `clap_mangen`. The following pages are generated:

| Page | Content |
|------|---------|
| `rings(1)` | Top-level usage, global options, command list |
| `rings-run(1)` | run subcommand reference |
| `rings-resume(1)` | resume subcommand reference |
| `rings-list(1)` | list subcommand reference |
| `rings-show(1)` | show subcommand reference |
| `rings-inspect(1)` | inspect subcommand reference (views, filters, output formats) |
| `rings-lineage(1)` | lineage subcommand reference |

Man pages are installed to `$PREFIX/share/man/man1/` during installation.

### Viewing

```bash
man rings
man rings-run
```

### Build-time generation

Man pages are generated at build time via a `build.rs` script that calls `clap_mangen` to emit `.1` files into `target/man/`. The packaging step copies these into the installation directory.

## Help String Requirements

Every flag, argument, and subcommand must have a help string. Review checklist:

- [ ] All subcommands: one-line description + paragraph-length long description
- [ ] All flags: what it does, what the default is if optional
- [ ] All arguments: what format is expected
- [ ] `--help` for top-level and each subcommand shows complete usage

Help strings are the source of truth — man pages and completion descriptions are generated from them.
