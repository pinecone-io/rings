# rings justfile — run `just` to see available recipes

default:
    @just --list

# Build debug binary
build:
    cargo build

# Build release binary
release:
    cargo build --release

# Build static release binary (musl)
release-static:
    cargo build --release --target x86_64-unknown-linux-musl

# Install release binary to ~/.local/bin
install: release
    mkdir -p ~/.local/bin
    cp target/release/rings ~/.local/bin/rings
    @echo "Installed to ~/.local/bin/rings"
    @rings --version

# Install static binary to ~/.local/bin
install-static: release-static
    mkdir -p ~/.local/bin
    cp target/x86_64-unknown-linux-musl/release/rings ~/.local/bin/rings
    @echo "Installed static binary to ~/.local/bin/rings"
    @rings --version

# Run all tests
test:
    cargo test --features testing

# Run tests without feature flag (skips engine integration)
test-fast:
    cargo test

# Check formatting
fmt-check:
    cargo fmt --check

# Fix formatting
fmt:
    cargo fmt

# Run clippy
lint:
    cargo clippy -- -D warnings

# Run all quality gates (matches CI)
validate: fmt-check lint test

# Watch for changes and rebuild
watch:
    cargo watch -x build
