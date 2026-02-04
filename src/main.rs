mod auth;
mod client;
mod config;
mod db;
mod types;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use db::{CrashDb, CrashFilters, CrashRow, NewCrash};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

// ─── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "asc-crash-fetcher",
    version,
    about = "Manage TestFlight crash feedback"
)]
struct Cli {
    /// Output format.
    #[arg(long, default_value = "text", global = true)]
    format: Format,

    /// Override data directory (default: ./asc-crashes or ~/.asc-crashes).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new data directory with template config and database.
    Init {
        /// Create in ~/.asc-crashes instead of ./asc-crashes.
        #[arg(long)]
        global: bool,
    },

    /// Verify API credentials and list visible apps.
    Apps,

    /// Pull new crashes from Apple and download logs.
    Sync {
        /// Sync only this app (bundle ID). Default: all configured apps.
        #[arg(long)]
        app: Option<String>,
    },

    /// List crashes.
    List {
        /// Filter by status (comma-separated: new,investigating,fixed,wontfix,duplicate).
        #[arg(long)]
        status: Option<String>,
        /// Show only crashes since this date (ISO 8601).
        #[arg(long)]
        since: Option<String>,
        /// Filter by app bundle ID.
        #[arg(long)]
        app: Option<String>,
        /// Max results.
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Show full details of a crash.
    Show { id: i64 },

    /// Print the absolute path to a crash log file.
    Log { id: i64 },

    /// Mark a crash as fixed.
    Fix {
        id: i64,
        #[arg(long)]
        notes: Option<String>,
    },

    /// Mark a crash as under investigation.
    Investigate { id: i64 },

    /// Mark a crash as won't fix.
    Wontfix {
        id: i64,
        #[arg(long)]
        notes: Option<String>,
    },

    /// Mark a crash as a duplicate of another.
    Duplicate {
        id: i64,
        /// The ID of the original crash.
        #[arg(long = "of")]
        of_id: i64,
    },

    /// Reset a crash status to "new".
    Reopen { id: i64 },

    /// Show crash statistics.
    Stats {
        #[arg(long)]
        app: Option<String>,
    },
}

// ─── Entry ───────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "asc_crash_fetcher=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // `init` doesn't need an existing data dir
    if let Cmd::Init { global } = &cli.cmd {
        return cmd_init(*global);
    }

    let data_dir = config::resolve_data_dir(cli.data_dir.as_deref())?;
    if !data_dir.join("config.toml").exists() {
        anyhow::bail!(
            "No config found. Run `asc-crash-fetcher init` first.\n\
             Looked in: {}",
            data_dir.display()
        );
    }

    let cfg = config::Config::load(&data_dir)?;
    let db_path = data_dir.join("crashes.db");
    let db = CrashDb::open(&db_path)?;
    let logs_dir = data_dir.join("logs");

    match cli.cmd {
        Cmd::Init { .. } => unreachable!(),
        Cmd::Apps => cmd_apps(&cfg, &cli.format).await,
        Cmd::Sync { app } => cmd_sync(&cfg, &db, &logs_dir, app.as_deref(), &cli.format).await,
        Cmd::List {
            status,
            since,
            app,
            limit,
        } => cmd_list(&db, status, since, app, limit, &cli.format),
        Cmd::Show { id } => cmd_show(&db, id, &cli.format),
        Cmd::Log { id } => cmd_log(&db, id),
        Cmd::Fix { id, notes } => cmd_status(&db, id, "fixed", notes.as_deref(), &cli.format),
        Cmd::Investigate { id } => cmd_status(&db, id, "investigating", None, &cli.format),
        Cmd::Wontfix { id, notes } => cmd_status(&db, id, "wontfix", notes.as_deref(), &cli.format),
        Cmd::Duplicate { id, of_id } => cmd_duplicate(&db, id, of_id, &cli.format),
        Cmd::Reopen { id } => cmd_reopen(&db, id, &cli.format),
        Cmd::Stats { app } => cmd_stats(&db, app.as_deref(), &cli.format),
    }
}

// ─── init ────────────────────────────────────────────────────────────────────

fn cmd_init(global: bool) -> Result<()> {
    let dir = config::init_data_dir(global)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(dir.join("logs"))?;

    let cfg_path = dir.join("config.toml");
    if cfg_path.exists() {
        eprintln!("Config already exists: {}", cfg_path.display());
    } else {
        std::fs::write(&cfg_path, config::CONFIG_TEMPLATE)?;
        eprintln!("Created {}", cfg_path.display());
    }

    // Touch the DB so migrate runs
    let _db = CrashDb::open(&dir.join("crashes.db"))?;

    eprintln!("Initialized in {}", dir.display());
    eprintln!();
    eprintln!("Next steps:");
    eprintln!("  1. Edit {} with your API credentials", cfg_path.display());
    eprintln!("  2. Run `asc-crash-fetcher apps` to verify");
    eprintln!("  3. Run `asc-crash-fetcher sync` to pull crashes");
    Ok(())
}

// ─── apps ────────────────────────────────────────────────────────────────────

async fn cmd_apps(cfg: &config::Config, fmt: &Format) -> Result<()> {
    let client = make_client(cfg)?;
    let apps = client.list_apps().await?;

    match fmt {
        Format::Json => {
            let out: Vec<serde_json::Value> = apps
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "id": a.id,
                        "bundle_id": a.attributes.as_ref().and_then(|x| x.bundle_id.as_deref()),
                        "name": a.attributes.as_ref().and_then(|x| x.name.as_deref()),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Text => {
            if apps.is_empty() {
                println!("No apps found for this API key.");
                return Ok(());
            }
            println!("{:<40} {:<30} NAME", "APP ID", "BUNDLE ID");
            println!("{}", "-".repeat(90));
            for a in &apps {
                let at = a.attributes.as_ref();
                println!(
                    "{:<40} {:<30} {}",
                    a.id,
                    at.and_then(|x| x.bundle_id.as_deref()).unwrap_or("-"),
                    at.and_then(|x| x.name.as_deref()).unwrap_or("-"),
                );
            }
        }
    }
    Ok(())
}

// ─── sync ────────────────────────────────────────────────────────────────────

async fn cmd_sync(
    cfg: &config::Config,
    db: &CrashDb,
    logs_dir: &Path,
    filter_app: Option<&str>,
    fmt: &Format,
) -> Result<()> {
    std::fs::create_dir_all(logs_dir)?;
    let client = make_client(cfg)?;

    let apps_to_sync: Vec<_> = if let Some(bundle) = filter_app {
        cfg.apps.iter().filter(|a| a.bundle_id == bundle).collect()
    } else {
        cfg.apps.iter().collect()
    };

    if apps_to_sync.is_empty() {
        anyhow::bail!("no matching apps found in config");
    }

    let mut all_new: Vec<serde_json::Value> = Vec::new();
    let mut all_recovered: Vec<serde_json::Value> = Vec::new();

    for app_cfg in &apps_to_sync {
        let asc_app = client
            .find_app(&app_cfg.bundle_id)
            .await?
            .with_context(|| {
                format!("app '{}' not found in App Store Connect", app_cfg.bundle_id)
            })?;

        let app_name = asc_app
            .attributes
            .as_ref()
            .and_then(|a| a.name.as_deref())
            .unwrap_or("unknown");

        let db_app_id = db.upsert_app(&app_cfg.bundle_id, Some(&asc_app.id), Some(app_name))?;

        if matches!(fmt, Format::Text) {
            eprintln!("Syncing {} ({})...", app_cfg.bundle_id, app_name);
        }

        // ── Fetch new crash submissions ──────────────────────────────────
        let mut new_crashes: Vec<CrashRow> = Vec::new();
        let mut url = client::AscClient::crash_list_url(&asc_app.id);
        let mut page = 0u32;

        'pagination: loop {
            page += 1;
            info!(page, app = %app_cfg.bundle_id, "fetching crash page");
            let resp = client.get_crash_page(&url).await?;
            let mut all_known_page = true;

            for sub in &resp.data {
                let attrs = sub.attributes.as_ref();
                let created = attrs
                    .and_then(|a| a.created_date)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default();

                let new_crash = NewCrash {
                    app_id: db_app_id,
                    submission_id: sub.id.clone(),
                    created_at: created,
                    device_model: attrs.and_then(|a| a.device_model.clone()),
                    os_version: attrs.and_then(|a| a.os_version.clone()),
                    app_platform: attrs.and_then(|a| a.app_platform.clone()),
                    architecture: attrs.and_then(|a| a.architecture.clone()),
                    tester_email: attrs.and_then(|a| a.email.clone()),
                    tester_comment: attrs.and_then(|a| a.comment.clone()),
                    bundle_id: attrs.and_then(|a| a.build_bundle_id.clone()),
                    build_id: sub
                        .relationships
                        .as_ref()
                        .and_then(|r| r.build.as_ref())
                        .and_then(|b| b.data.as_ref())
                        .map(|d| d.id.clone()),
                    app_uptime_ms: attrs.and_then(|a| a.app_uptime_in_milliseconds),
                    battery_pct: attrs.and_then(|a| a.battery_percentage),
                    connection_type: attrs.and_then(|a| a.connection_type.clone()),
                };

                if let Some(local_id) = db.insert_crash(&new_crash)? {
                    all_known_page = false;
                    if let Some(row) = db.get_crash(local_id)? {
                        new_crashes.push(row);
                    }
                }
            }

            // If every entry on this page was already known, we've caught up
            if all_known_page || resp.data.is_empty() {
                break 'pagination;
            }

            match resp.links.next {
                Some(next) => url = next,
                None => break 'pagination,
            }

            if page >= 50 {
                warn!("hit 50 page limit, stopping pagination");
                break;
            }
        }

        // ── Download logs for new + retry missing ────────────────────────
        let mut recovered: Vec<CrashRow> = Vec::new();
        let missing = db.crashes_missing_logs()?;

        for crash in &missing {
            match client.get_crash_log(&crash.submission_id).await {
                Ok(Some(text)) => {
                    let path = logs_dir.join(format!("{}.ips", crash.id));
                    let abs = std::fs::canonicalize(logs_dir)
                        .unwrap_or_else(|_| logs_dir.to_path_buf())
                        .join(format!("{}.ips", crash.id));
                    std::fs::write(&path, &text)?;
                    db.set_log(crash.id, &abs.to_string_lossy())?;

                    if let Some(c) = new_crashes.iter_mut().find(|c| c.id == crash.id) {
                        c.has_log = true;
                        c.log_path = Some(abs.to_string_lossy().to_string());
                    } else {
                        let mut updated = crash.clone();
                        updated.has_log = true;
                        updated.log_path = Some(abs.to_string_lossy().to_string());
                        recovered.push(updated);
                    }
                }
                Ok(None) => {} // not available yet
                Err(e) => {
                    warn!(id = crash.id, err = %e, "failed to download crash log");
                }
            }
        }

        // ── Output ───────────────────────────────────────────────────────
        match fmt {
            Format::Text => {
                for c in &new_crashes {
                    eprintln!(
                        "  [NEW] #{:<4} {} / {}  {}",
                        c.id,
                        c.device_model.as_deref().unwrap_or("?"),
                        c.os_version.as_deref().unwrap_or("?"),
                        &c.created_at[..19.min(c.created_at.len())],
                    );
                    if let Some(ref p) = c.log_path {
                        eprintln!("         → {p}");
                    } else {
                        eprintln!("         → (log not available yet)");
                    }
                }
                for c in &recovered {
                    eprintln!(
                        "  [LOG] #{:<4} → {}",
                        c.id,
                        c.log_path.as_deref().unwrap_or("?")
                    );
                }
                if !new_crashes.is_empty() || !recovered.is_empty() {
                    let log_count =
                        new_crashes.iter().filter(|c| c.has_log).count() + recovered.len();
                    eprintln!(
                        "  {} new crash(es), {} log(s) downloaded",
                        new_crashes.len(),
                        log_count
                    );
                }
            }
            Format::Json => {
                for c in &new_crashes {
                    all_new.push(crash_to_json(c));
                }
                for c in &recovered {
                    all_recovered.push(serde_json::json!({
                        "id": c.id,
                        "log_path": c.log_path,
                    }));
                }
            }
        }
    }

    let total = db.count_total()?;
    let unfixed = db.count_unfixed()?;

    match fmt {
        Format::Text => {
            eprintln!("Total: {total} crashes ({unfixed} unfixed)");
        }
        Format::Json => {
            let out = serde_json::json!({
                "new_crashes": all_new,
                "recovered_logs": all_recovered,
                "total": total,
                "unfixed": unfixed,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
    }

    Ok(())
}

// ─── list ────────────────────────────────────────────────────────────────────

fn cmd_list(
    db: &CrashDb,
    status: Option<String>,
    since: Option<String>,
    app: Option<String>,
    limit: usize,
    fmt: &Format,
) -> Result<()> {
    let filters = CrashFilters {
        status: status.map(|s| s.split(',').map(|x| x.trim().to_string()).collect()),
        since,
        app_bundle_id: app,
        limit,
    };
    let crashes = db.list_crashes(&filters)?;

    match fmt {
        Format::Json => {
            let out = serde_json::json!({
                "crashes": crashes,
                "count": crashes.len(),
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        Format::Text => {
            if crashes.is_empty() {
                println!("No crashes found.");
                return Ok(());
            }
            println!(
                " {:<5} {:<14} {:<20} {:<14} {:<10} APP",
                "ID", "STATUS", "DATE", "DEVICE", "OS"
            );
            println!("{}", "-".repeat(90));
            for c in &crashes {
                let date = if c.created_at.len() >= 19 {
                    &c.created_at[..19]
                } else {
                    &c.created_at
                };
                println!(
                    " {:<5} {:<14} {:<20} {:<14} {:<10} {}",
                    c.id,
                    c.status,
                    date,
                    c.device_model.as_deref().unwrap_or("-"),
                    c.os_version.as_deref().unwrap_or("-"),
                    c.app_bundle_id.as_deref().unwrap_or("-"),
                );
            }
            println!();
            let unfixed = crashes
                .iter()
                .filter(|c| c.status == "new" || c.status == "investigating")
                .count();
            println!("{} crash(es) shown ({unfixed} unfixed)", crashes.len());
        }
    }
    Ok(())
}

// ─── show ────────────────────────────────────────────────────────────────────

fn cmd_show(db: &CrashDb, id: i64, fmt: &Format) -> Result<()> {
    let crash = db
        .get_crash(id)?
        .with_context(|| format!("crash #{id} not found"))?;

    match fmt {
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(&crash)?);
        }
        Format::Text => {
            println!("Crash #{}", crash.id);
            println!("{}", "─".repeat(40));
            println!("Status:     {}", crash.status);
            println!("Created:    {}", crash.created_at);
            println!("Synced:     {}", crash.synced_at);
            if let Some(ref v) = crash.device_model {
                println!("Device:     {v}");
            }
            if let Some(ref v) = crash.os_version {
                println!("OS:         {v}");
            }
            if let Some(ref v) = crash.app_platform {
                println!("Platform:   {v}");
            }
            if let Some(ref v) = crash.architecture {
                println!("Arch:       {v}");
            }
            if let Some(ref v) = crash.tester_email {
                if !v.is_empty() {
                    println!("Tester:     {v}");
                }
            }
            if let Some(ref v) = crash.tester_comment {
                if !v.is_empty() {
                    println!("Comment:    {v}");
                }
            }
            if let Some(ref v) = crash.app_bundle_id {
                println!("App:        {v}");
            }
            if let Some(ref v) = crash.app_name {
                println!("App Name:   {v}");
            }
            if let Some(v) = crash.app_uptime_ms {
                println!("Uptime:     {:.1}s", v as f64 / 1000.0);
            }
            if let Some(v) = crash.battery_pct {
                println!("Battery:    {v}%");
            }
            if let Some(ref v) = crash.connection_type {
                println!("Connection: {v}");
            }
            if let Some(ref v) = crash.fix_notes {
                println!("Fix Notes:  {v}");
            }
            if let Some(ref v) = crash.fixed_at {
                println!("Fixed At:   {v}");
            }
            if let Some(v) = crash.duplicate_of {
                println!("Dup Of:     #{v}");
            }

            if let Some(ref p) = crash.log_path {
                println!("Log:        {p}");
                if let Ok(text) = std::fs::read_to_string(p) {
                    println!();
                    println!("--- Crash log (first 50 lines) ---");
                    for line in text.lines().take(50) {
                        println!("{line}");
                    }
                    let total_lines = text.lines().count();
                    if total_lines > 50 {
                        println!("... ({} more lines)", total_lines - 50);
                    }
                }
            } else {
                println!("Log:        (not available)");
            }
        }
    }
    Ok(())
}

// ─── log (just prints the path) ──────────────────────────────────────────────

fn cmd_log(db: &CrashDb, id: i64) -> Result<()> {
    let crash = db
        .get_crash(id)?
        .with_context(|| format!("crash #{id} not found"))?;
    match crash.log_path {
        Some(ref p) => {
            println!("{p}");
            Ok(())
        }
        None => {
            eprintln!("crash #{id}: no log available");
            std::process::exit(1);
        }
    }
}

// ─── status changes ──────────────────────────────────────────────────────────

fn cmd_status(
    db: &CrashDb,
    id: i64,
    status: &str,
    notes: Option<&str>,
    fmt: &Format,
) -> Result<()> {
    if !db.update_status(id, status, notes)? {
        anyhow::bail!("crash #{id} not found");
    }
    let crash = db.get_crash(id)?.unwrap();
    match fmt {
        Format::Json => println!("{}", serde_json::to_string_pretty(&crash)?),
        Format::Text => eprintln!("Crash #{id} marked as {status}"),
    }
    Ok(())
}

fn cmd_duplicate(db: &CrashDb, id: i64, of_id: i64, fmt: &Format) -> Result<()> {
    db.get_crash(of_id)?
        .with_context(|| format!("target crash #{of_id} not found"))?;
    if !db.mark_duplicate(id, of_id)? {
        anyhow::bail!("crash #{id} not found");
    }
    let crash = db.get_crash(id)?.unwrap();
    match fmt {
        Format::Json => println!("{}", serde_json::to_string_pretty(&crash)?),
        Format::Text => eprintln!("Crash #{id} marked as duplicate of #{of_id}"),
    }
    Ok(())
}

fn cmd_reopen(db: &CrashDb, id: i64, fmt: &Format) -> Result<()> {
    if !db.reopen(id)? {
        anyhow::bail!("crash #{id} not found");
    }
    let crash = db.get_crash(id)?.unwrap();
    match fmt {
        Format::Json => println!("{}", serde_json::to_string_pretty(&crash)?),
        Format::Text => eprintln!("Crash #{id} reopened"),
    }
    Ok(())
}

// ─── stats ───────────────────────────────────────────────────────────────────

fn cmd_stats(db: &CrashDb, app: Option<&str>, fmt: &Format) -> Result<()> {
    let stats = db.stats(app)?;

    match fmt {
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        Format::Text => {
            println!("Crash Statistics");
            println!("{}", "─".repeat(30));
            println!("Total:          {}", stats.total);
            for status in &["new", "investigating", "fixed", "wontfix", "duplicate"] {
                let n = stats.by_status.get(*status).copied().unwrap_or(0);
                if n > 0 {
                    println!("{:<16}{}", format!("{status}:"), n);
                }
            }
            println!("Unfixed:        {}", stats.unfixed);

            if !stats.by_device.is_empty() {
                println!();
                println!("By Device:");
                for (device, count) in &stats.by_device {
                    println!("  {:<20} {count}", device);
                }
            }

            if !stats.by_os.is_empty() {
                println!();
                println!("By OS:");
                for (os, count) in &stats.by_os {
                    println!("  {:<20} {count}", os);
                }
            }
        }
    }
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_client(cfg: &config::Config) -> Result<client::AscClient> {
    client::AscClient::new(
        cfg.api.issuer_id.clone(),
        cfg.api.key_id.clone(),
        cfg.api.private_key.clone(),
    )
}

fn crash_to_json(c: &CrashRow) -> serde_json::Value {
    serde_json::json!({
        "id": c.id,
        "submission_id": c.submission_id,
        "created_at": c.created_at,
        "device_model": c.device_model,
        "os_version": c.os_version,
        "app_platform": c.app_platform,
        "architecture": c.architecture,
        "tester_email": c.tester_email,
        "tester_comment": c.tester_comment,
        "bundle_id": c.bundle_id,
        "has_log": c.has_log,
        "log_path": c.log_path,
        "status": c.status,
        "app_bundle_id": c.app_bundle_id,
        "app_name": c.app_name,
    })
}
