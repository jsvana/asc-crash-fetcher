# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust CLI tool for managing TestFlight crash feedback from the App Store Connect API. Pulls crash submissions into a local SQLite database, downloads `.ips` crash logs, and tracks fix status. Designed for both human and AI agent use (all commands support `--format json`).

Uses the TestFlight Feedback API (WWDC25) — `betaFeedbackCrashSubmissions` and `betaCrashLogs` endpoints from App Store Connect API v4.2.

## Build & Development Commands

```bash
cargo build                           # Debug build
cargo build --release                 # Release build
cargo test                            # Run all tests
cargo test help_works                 # Run single test
cargo fmt                             # Format code
cargo clippy -- -D warnings           # Lint (CI enforces zero warnings)
RUST_LOG=debug cargo run -- sync      # Run with debug logging
```

## Architecture

```
src/
├── main.rs    # CLI entry point (clap), command handlers, output formatting
├── auth.rs    # ES256 JWT generation for App Store Connect API auth
├── client.rs  # HTTP client for ASC API v4.2 (reqwest + JSON:API pagination)
├── config.rs  # TOML config loading, data directory resolution
├── db.rs      # SQLite database (rusqlite), schema migrations, queries
└── types.rs   # ASC API response types (serde deserialize)
```

**Data flow**: `main.rs` parses CLI → loads `Config` → opens `CrashDb` → uses `AscClient` to fetch from API → stores in SQLite → downloads logs to `logs/{id}.ips`

**Data directory resolution** (priority order):
1. `--data-dir PATH` explicit override
2. `./asc-crashes/` project-local if `config.toml` exists
3. `~/.asc-crashes/` global fallback

## Key Patterns

- All user-facing output supports `--format json` for machine consumption
- Status messages go to stderr, structured data to stdout
- Status workflow: `new` → `investigating` → `fixed` / `wontfix` / `duplicate`
- Missing logs are retried on every `sync` until Apple's 120-day expiry
- JWT tokens are short-lived (20 min) and regenerated per request

## Testing

Integration tests in `tests/cli.rs` run the actual binary against temp directories with mock configs. Tests don't hit the real API — they verify CLI behavior, init flow, and error handling.

## CI Requirements

- `cargo fmt --all -- --check` (formatting)
- `cargo clippy --all-targets --all-features -- -D warnings` (zero warnings)
- `cargo test --all-features` (Linux + macOS)
