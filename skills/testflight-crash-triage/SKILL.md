---
name: testflight-crash-triage
description: >
  Triage TestFlight crash reports and screenshot feedback from App Store Connect
  using the asc-crash-fetcher CLI. Syncs crash submissions and feedback to a local
  SQLite database, downloads .ips crash logs and screenshot attachments, analyzes
  stack traces, and tracks fix status. Use when the user mentions TestFlight crashes,
  crash triage, .ips crash logs, TestFlight feedback, screenshot submissions,
  App Store Connect crash data, or asks to analyze iOS/macOS crash reports.
---

# TestFlight Crash Triage

Analyze and manage TestFlight crash reports and screenshot feedback using the
`asc-crash-fetcher` CLI. This tool maintains a local SQLite database of crash
submissions and feedback, downloading `.ips` crash logs and screenshots to disk
for analysis.

## Prerequisites

The `asc-crash-fetcher` binary must be on PATH. Data lives in `./asc-crashes/`
(project-local) or `~/.asc-crashes/` (global). Run `asc-crash-fetcher init`
if no data directory exists yet.

## Crash Workflow

### 1. Sync latest crashes and feedback

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
  "new_feedbacks": [
    {
      "id": 3,
      "device_model": "iPhone14,2",
      "os_version": "iOS 18.1",
      "screenshot_path": "/Users/jay/.asc-crashes/screenshots/3.png",
      "tester_comment": "Button text is cut off"
    }
  ],
  "recovered_screenshots": [],
  "crash_total": 47,
  "crash_unfixed": 12,
  "feedback_total": 8,
  "feedback_unfixed": 5
}
```

Sync only crashes or only feedback:
```bash
asc-crash-fetcher sync --no-feedback --format json   # crashes only
asc-crash-fetcher sync --no-crashes --format json    # feedback only
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

## Feedback Workflow

Screenshot feedback submissions follow the same pattern as crashes but use the
`feedback` subcommand.

### 1. List unfixed feedback

```bash
asc-crash-fetcher feedback list --status new,investigating --format json
```

### 2. View feedback details

```bash
asc-crash-fetcher feedback show <id> --format json
```

The `screenshot_path` field contains the absolute path to the screenshot image.
View it with an image viewer or the `Read` tool (which can display images).

### 3. Get screenshot path

```bash
asc-crash-fetcher feedback screenshot <id>
```

Prints only the absolute file path — useful for piping to other tools.

### 4. Mark resolution

```bash
asc-crash-fetcher feedback fix <id> --notes "Fixed button layout in iOS 18"
asc-crash-fetcher feedback investigate <id>
asc-crash-fetcher feedback wontfix <id> --notes "Expected behavior on small screens"
asc-crash-fetcher feedback duplicate <id> --of <other_id>
asc-crash-fetcher feedback reopen <id>
```

### 5. Review feedback stats

```bash
asc-crash-fetcher feedback stats --format json
```

## Command Reference

### Crash Commands

| Task | Command |
|---|---|
| Pull new crashes | `asc-crash-fetcher sync --format json` |
| Pull crashes only | `asc-crash-fetcher sync --no-feedback --format json` |
| List unfixed | `asc-crash-fetcher list --status new,investigating --format json` |
| Show one crash | `asc-crash-fetcher show <id> --format json` |
| Get log path | `asc-crash-fetcher log <id>` |
| Mark fixed | `asc-crash-fetcher fix <id> --notes "description"` |
| Mark investigating | `asc-crash-fetcher investigate <id>` |
| Mark won't fix | `asc-crash-fetcher wontfix <id> --notes "reason"` |
| Mark duplicate | `asc-crash-fetcher duplicate <id> --of <other_id>` |
| Reopen | `asc-crash-fetcher reopen <id>` |
| Statistics | `asc-crash-fetcher stats --format json` |

### Feedback Commands

| Task | Command |
|---|---|
| Pull feedback only | `asc-crash-fetcher sync --no-crashes --format json` |
| List unfixed | `asc-crash-fetcher feedback list --status new,investigating --format json` |
| Show one feedback | `asc-crash-fetcher feedback show <id> --format json` |
| Get screenshot path | `asc-crash-fetcher feedback screenshot <id>` |
| Mark fixed | `asc-crash-fetcher feedback fix <id> --notes "description"` |
| Mark investigating | `asc-crash-fetcher feedback investigate <id>` |
| Mark won't fix | `asc-crash-fetcher feedback wontfix <id> --notes "reason"` |
| Mark duplicate | `asc-crash-fetcher feedback duplicate <id> --of <other_id>` |
| Reopen | `asc-crash-fetcher feedback reopen <id>` |
| Statistics | `asc-crash-fetcher feedback stats --format json` |

## Important Notes

- Always use `--format json` for structured output.
- The `log <id>` and `feedback screenshot <id>` commands print ONLY the absolute file path — useful for piping.
- Crash logs and screenshots may not be available immediately. `sync` retries missing files each run.
- Reports expire after 120 days on Apple's servers.
- Status values: `new`, `investigating`, `fixed`, `wontfix`, `duplicate`.
- Use `--data-dir` to override the default data directory.
- Use `--app BUNDLE_ID` to filter sync/list/stats to a single app.
- Screenshots are saved as `.png`, `.jpg`, `.heic`, `.mov`, or `.mp4` based on MIME type.
