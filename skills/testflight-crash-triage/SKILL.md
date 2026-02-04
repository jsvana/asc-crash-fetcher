---
name: testflight-crash-triage
description: >
  Triage TestFlight crash reports from App Store Connect using the
  asc-crash-fetcher CLI. Syncs crash submissions to a local SQLite database,
  downloads .ips crash logs, analyzes stack traces, and tracks fix status.
  Use when the user mentions TestFlight crashes, crash triage, .ips crash logs,
  App Store Connect crash data, or asks to analyze iOS/macOS crash reports.
---

# TestFlight Crash Triage

Analyze and manage TestFlight crash reports using the `asc-crash-fetcher` CLI.
This tool maintains a local SQLite database of crash submissions and downloads
`.ips` crash logs to disk for analysis.

## Prerequisites

The `asc-crash-fetcher` binary must be on PATH. Data lives in `./asc-crashes/`
(project-local) or `~/.asc-crashes/` (global). Run `asc-crash-fetcher init`
if no data directory exists yet.

## Workflow

### 1. Sync latest crashes

Always start by pulling the latest data:

```bash
asc-crash-fetcher sync --format json
```

Returns:
```json
{
  "new_crashes": [
    {
      "id": 14,
      "device_model": "iPhone15,3",
      "os_version": "iOS 18.2",
      "log_path": "/Users/jay/.asc-crashes/logs/14.ips",
      "tester_comment": "Crashed when opening settings"
    }
  ],
  "recovered_logs": [],
  "total": 47,
  "unfixed": 12
}
```

### 2. List unfixed crashes

```bash
asc-crash-fetcher list --status new,investigating --format json
```

Narrow by app:
```bash
asc-crash-fetcher list --status new --app com.example.myapp --format json
```

### 3. Read a crash log

Get the crash details:
```bash
asc-crash-fetcher show <id> --format json
```

The `log_path` field contains the absolute path to the `.ips` crash log.
Read it with the `view` or `Read` tool. Crash log files are named `{id}.ips`
in the `logs/` directory, keyed by local database ID.

### 4. Analyze the crash

When reading a `.ips` crash log, focus on:

1. **Exception Type** — `EXC_BAD_ACCESS (SIGSEGV)` = null pointer or
   use-after-free. `EXC_CRASH (SIGABRT)` = assertion or uncaught exception.
2. **Crashed Thread** — find the thread marked `Crashed` and walk up the
   frames to the first frame in user code (not Apple frameworks).
3. **Termination Reason** — often the most readable description, especially
   for Swift runtime errors.
4. **Last Exception Backtrace** — for uncaught NSException/Swift errors,
   this is usually more useful than the crashed thread.
5. **Pattern matching** — multiple crashes with the same crashed function
   or exception type are likely the same root cause. Link them with the
   `duplicate` command.

### 5. Mark resolution

```bash
asc-crash-fetcher fix <id> --notes "Fixed nil unwrap in SettingsViewController.swift:42"
```

Other status transitions:
```bash
asc-crash-fetcher investigate <id>
asc-crash-fetcher wontfix <id> --notes "Only on jailbroken devices"
asc-crash-fetcher duplicate <id> --of <other_id>
asc-crash-fetcher reopen <id>
```

### 6. Review stats

```bash
asc-crash-fetcher stats --format json
```

## Command Reference

| Task | Command |
|---|---|
| Pull new crashes | `asc-crash-fetcher sync --format json` |
| List unfixed | `asc-crash-fetcher list --status new,investigating --format json` |
| Show one crash | `asc-crash-fetcher show <id> --format json` |
| Get log path | `asc-crash-fetcher log <id>` |
| Mark fixed | `asc-crash-fetcher fix <id> --notes "description"` |
| Mark investigating | `asc-crash-fetcher investigate <id>` |
| Mark won't fix | `asc-crash-fetcher wontfix <id> --notes "reason"` |
| Mark duplicate | `asc-crash-fetcher duplicate <id> --of <other_id>` |
| Reopen | `asc-crash-fetcher reopen <id>` |
| Statistics | `asc-crash-fetcher stats --format json` |

## Important Notes

- Always use `--format json` for structured output.
- The `log <id>` command prints ONLY the absolute file path — useful for piping.
- Crash logs may not be available immediately. `sync` retries missing logs each run.
- Reports expire after 120 days on Apple's servers.
- Status values: `new`, `investigating`, `fixed`, `wontfix`, `duplicate`.
- Use `--data-dir` to override the default data directory.
- Use `--app BUNDLE_ID` to filter sync/list/stats to a single app.
