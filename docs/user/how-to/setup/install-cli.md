---
title: Install the CLI
description: How to get the rr CLI binary onto your machine
---

# Install the CLI

The `rr` CLI lets you initialise configuration, test commit parsing, and simulate webhook events
locally without running the server.

## Option 1: Cargo (recommended if you have Rust installed)

```bash
cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli
```

This builds from source and places the `rr` binary in `~/.cargo/bin/`. Make sure that directory
is on your `PATH`.

To update to the latest version, run the same command again with the `--force` flag:

```bash
cargo install --git https://github.com/pvandervelde/release_regent.git release-regent-cli --force
```

## Option 2: Pre-built binary

1. Go to the [releases page](https://github.com/pvandervelde/release_regent/releases).
2. Download the archive for your platform:
   - `rr-x86_64-unknown-linux-gnu.tar.gz` — Linux (x86-64)
   - `rr-aarch64-unknown-linux-gnu.tar.gz` — Linux (ARM64)
   - `rr-x86_64-apple-darwin.tar.gz` — macOS (Intel)
   - `rr-aarch64-apple-darwin.tar.gz` — macOS (Apple Silicon)
   - `rr-x86_64-pc-windows-msvc.zip` — Windows (x86-64)
3. Extract the archive and move the `rr` (or `rr.exe`) binary to a directory on your `PATH`.

## Verify the installation

```bash
rr --version
```

Expected output:

```
rr 0.3.0
```

If you see a "command not found" error, check that the binary location is included in your
`PATH` environment variable.

## Shell completion (optional)

The CLI can generate shell completion scripts:

```bash
# bash
rr --generate-completion bash >> ~/.bash_completion

# zsh
rr --generate-completion zsh > ~/.zfunc/_rr

# fish
rr --generate-completion fish > ~/.config/fish/completions/rr.fish

# PowerShell
rr --generate-completion powershell >> $PROFILE
```

---

## Next steps

- [Set up the GitHub App](github-app-setup.md) to connect Release Regent to your repository
- [Your first automated release](../../tutorials/01-first-release.md) for a complete walkthrough
