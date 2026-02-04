# asc-crash-fetcher

Manage TestFlight crash feedback from the App Store Connect API. Pulls crash
submissions into a local SQLite database, downloads `.ips` crash logs to
predictable paths, and tracks fix status — designed to be driven by both
humans and AI agents.

Uses the **TestFlight Feedback API** (WWDC25) — `betaFeedbackCrashSubmissions`
and `betaCrashLogs` endpoints from the App Store Connect API v4.2.

## Features

- **Multi-app support** — monitor multiple bundle IDs from a single config
- **SQLite crash database** — local, queryable, zero infrastructure
- **Automatic log download** — `.ips` crash logs saved with stable integer filenames
- **Retry missing logs** — re-attempts download on every sync until the 120-day expiry
- **Status tracking** — `new` → `investigating` → `fixed` / `wontfix` / `duplicate`
- **JSON output** — every command supports `--format json` for machine consumption
- **Claude Code skill** — included SKILL.md for AI-assisted crash triage
- **Project-local or global** — `./asc-crashes/` per-project or `~/.asc-crashes/` global

## Quick Start

### Install from GitHub Releases

Download a prebuilt binary from [Releases](https://github.com/w6jsv/asc-crash-fetcher/releases)
and place it on your PATH.

### Install from source

```bash
cargo install --git https://github.com/w6jsv/asc-crash-fetcher
```

### Initialize and run

```bash
# Create a project-local data directory
asc-crash-fetcher init

# Or create a global one in ~/.asc-crashes
asc-crash-fetcher init --global

# Add your API credentials
$EDITOR asc-crashes/config.toml

# Verify your key works
asc-crash-fetcher apps

# Pull crashes and download logs
asc-crash-fetcher sync

# See what's unfixed
asc-crash-fetcher list --status new,investigating

# Look at a specific crash
asc-crash-fetcher show 3

# Mark it fixed
asc-crash-fetcher fix 3 --notes "Nil check in SettingsVC.swift:42"
```

## Data Directory

The tool resolves its data directory in priority order:

1. `--data-dir PATH` — explicit override
2. `./asc-crashes/` — project-local (if `config.toml` exists there)
3. `~/.asc-crashes/` — global fallback

```
asc-crashes/
├── config.toml     # API credentials + app list
├── crashes.db      # SQLite database
└── logs/
    ├── 1.ips       # Crash logs keyed by local DB id
    ├── 2.ips
    └── ...
```

## Configuration

```toml
[api]
issuer_id   = "your-issuer-id"
key_id      = "ABCDEF1234"
private_key = "AuthKey_ABCDEF1234.p8"  # path relative to data dir, absolute, or inline PEM

# Monitor one or more apps
[[apps]]
bundle_id = "com.example.app1"

[[apps]]
bundle_id = "com.example.app2"
```

Get your API key from [App Store Connect → Integrations → App Store Connect API](https://appstoreconnect.apple.com/access/integrations/api).
The key needs at least **App Manager** role.

## Commands

| Command | Description |
|---|---|
| `init [--global]` | Create data directory with template config |
| `apps` | List apps visible to your API key |
| `sync [--app BUNDLE]` | Pull new crashes, download logs |
| `list [--status S] [--since DATE] [--app BUNDLE] [--limit N]` | List crashes with filters |
| `show <id>` | Full crash details + log preview |
| `log <id>` | Print absolute path to the `.ips` file |
| `fix <id> [--notes "…"]` | Mark as fixed |
| `investigate <id>` | Mark as under investigation |
| `wontfix <id> [--notes "…"]` | Mark as won't fix |
| `duplicate <id> --of <other>` | Mark as duplicate of another crash |
| `reopen <id>` | Reset status to "new" |
| `stats [--app BUNDLE]` | Counts by status, device, OS |

All commands accept `--format json` for structured output.

## JSON Output

Every command supports `--format json`, printing structured data to stdout
while keeping status messages on stderr:

```bash
asc-crash-fetcher sync --format json | jq '.new_crashes[].log_path'
asc-crash-fetcher list --status new --format json | jq '.crashes[] | {id, device_model, os_version}'
asc-crash-fetcher stats --format json
```

## Claude Code Integration

This project ships as a [Claude Code plugin](https://code.claude.com/docs/en/plugins)
with an Agent Skill for AI-assisted crash triage.

### Install via Claude Code plugin marketplace

```
/plugin marketplace add w6jsv/asc-crash-fetcher
/plugin install testflight-crash-triage@asc-crash-fetcher
```

### Or install the skill manually

```bash
# Personal skill (all projects)
mkdir -p ~/.claude/skills/testflight-crash-triage
cp skills/testflight-crash-triage/SKILL.md ~/.claude/skills/testflight-crash-triage/

# Project skill (this project only)
mkdir -p .claude/skills/testflight-crash-triage
cp skills/testflight-crash-triage/SKILL.md .claude/skills/testflight-crash-triage/
```

Once installed, Claude will automatically use the skill when you ask about
TestFlight crashes, crash triage, or `.ips` crash logs. You can also be direct:

```
Sync my TestFlight crashes and analyze any new ones.
```

## Prerequisites

- **App Store Connect API Key** — at least App Manager role
- **Active TestFlight testers** — with crash feedback enabled in the TestFlight app
- **Rust 1.75+** — if building from source

Crash reports are retained by Apple for **120 days** after submission.

## Logging

```bash
RUST_LOG=debug asc-crash-fetcher sync
RUST_LOG=asc_crash_fetcher=trace asc-crash-fetcher list
```

All log output goes to stderr so it doesn't interfere with `--format json` on stdout.

## Contributing

Contributions welcome! Please open an issue to discuss before sending a PR
for anything substantial.

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## License

[MIT](LICENSE)
