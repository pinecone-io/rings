# Distribution and Build

## Design Goal

rings should be trivially installable: download a single binary, make it executable, done. No runtime dependencies, no package manager required.

## Static Binary

rings is built as a fully statically-linked binary using musl libc on Linux. This eliminates glibc version compatibility issues and allows the binary to run on any Linux system regardless of its libc version.

On macOS, dynamic linking against the system frameworks is acceptable (the system libraries are stable ABI); however the binary should link no third-party shared libraries.

## Target Platforms

Initial release targets:

| Platform | Rust Target Triple | Notes |
|----------|--------------------|-------|
| Linux x86_64 | `x86_64-unknown-linux-musl` | Statically linked, runs on any Linux |
| Linux aarch64 | `aarch64-unknown-linux-musl` | For ARM servers and Apple Silicon Linux |
| macOS x86_64 | `x86_64-apple-darwin` | Intel Mac |
| macOS aarch64 | `aarch64-apple-darwin` | Apple Silicon |

A macOS universal binary (fat binary combining x86_64 + aarch64) is produced by linking the two macOS builds with `lipo`:

```bash
lipo -create -output rings-macos \
  target/x86_64-apple-darwin/release/rings \
  target/aarch64-apple-darwin/release/rings
```

## Building for Release

```bash
# Linux static (musl)
cargo build --release --target x86_64-unknown-linux-musl

# macOS universal
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
lipo -create -output rings \
  target/x86_64-apple-darwin/release/rings \
  target/aarch64-apple-darwin/release/rings
```

## Build Dependencies

The Rust toolchain is the only build dependency. No C compiler, no system libraries, no pkg-config. All Rust dependencies are pure Rust or have bundled C sources compiled at build time.

To verify no dynamic dependencies on Linux:
```bash
ldd target/x86_64-unknown-linux-musl/release/rings
# Expected: "not a dynamic executable"

file target/x86_64-unknown-linux-musl/release/rings
# Expected: ELF 64-bit LSB executable, statically linked
```

## Binary Size

The release binary should be stripped and optimized. Add to `Cargo.toml`:

```toml
[profile.release]
strip = true
opt-level = "z"     # optimize for size
lto = true
codegen-units = 1
```

Target binary size: < 5 MB for the stripped release binary.

## Installation (end user)

```bash
# Linux
curl -L https://github.com/owner/rings/releases/latest/download/rings-linux-x86_64 -o rings
chmod +x rings
sudo mv rings /usr/local/bin/

# macOS (universal)
curl -L https://github.com/owner/rings/releases/latest/download/rings-macos -o rings
chmod +x rings
sudo mv rings /usr/local/bin/
```

No additional steps. The binary is self-contained.

## CI Release Pipeline

On git tag push (e.g. `v0.1.0`), a GitHub Actions workflow:

1. Builds for all four target triples using cross-compilation
2. Creates the macOS universal binary with `lipo`
3. Strips all binaries
4. Creates a GitHub Release with binaries attached
5. Names binaries: `rings-linux-x86_64`, `rings-linux-aarch64`, `rings-macos`

## Cargo Install

For users with Rust installed:

```bash
cargo install rings
```

This builds from source for the host platform. The `Cargo.toml` must not have any platform-specific build requirements that would prevent this from working.

## Release Verification

### Checksums

SHA256 checksums are published alongside each GitHub Release as `checksums.txt`. Users should verify the download before executing:

```bash
# Download binary and checksum file
curl -LO https://github.com/owner/rings/releases/latest/download/rings-linux-x86_64
curl -LO https://github.com/owner/rings/releases/latest/download/checksums.txt

# Verify
sha256sum --check --ignore-missing checksums.txt
```

### GPG Signatures

All releases are GPG-signed. The signature file is `checksums.txt.asc`. The maintainer's public key fingerprint and keyserver location are documented in `SECURITY.md` in the repository.

```bash
# Verify GPG signature
gpg --verify checksums.txt.asc checksums.txt
```

### Build Reproducibility

`Cargo.lock` is committed to version control and CI uses a pinned Rust toolchain version (documented in `rust-toolchain.toml`). This ensures reproducible dependency resolution across builds. All builds in CI use identical dependency versions.
