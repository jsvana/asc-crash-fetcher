//! SQLite crash database.
//!
//! Stores crash metadata, status, and log paths. Designed to be queried
//! by both the CLI and a Claude skill via `--format json`.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

pub struct CrashDb {
    conn: Connection,
}

// ─── Row types (serializable for JSON output) ────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct CrashRow {
    pub id: i64,
    pub app_id: i64,
    pub submission_id: String,
    pub created_at: String,
    pub synced_at: String,
    pub device_model: Option<String>,
    pub os_version: Option<String>,
    pub app_platform: Option<String>,
    pub architecture: Option<String>,
    pub tester_email: Option<String>,
    pub tester_comment: Option<String>,
    pub bundle_id: Option<String>,
    pub build_id: Option<String>,
    pub app_uptime_ms: Option<i64>,
    pub battery_pct: Option<i32>,
    pub connection_type: Option<String>,
    pub has_log: bool,
    pub log_path: Option<String>,
    pub status: String,
    pub fixed_at: Option<String>,
    pub fix_notes: Option<String>,
    pub duplicate_of: Option<i64>,
    // Joined from apps table
    pub app_bundle_id: Option<String>,
    pub app_name: Option<String>,
}

pub struct NewCrash {
    pub app_id: i64,
    pub submission_id: String,
    pub created_at: String,
    pub device_model: Option<String>,
    pub os_version: Option<String>,
    pub app_platform: Option<String>,
    pub architecture: Option<String>,
    pub tester_email: Option<String>,
    pub tester_comment: Option<String>,
    pub bundle_id: Option<String>,
    pub build_id: Option<String>,
    pub app_uptime_ms: Option<i64>,
    pub battery_pct: Option<i32>,
    pub connection_type: Option<String>,
}

pub struct CrashFilters {
    pub status: Option<Vec<String>>,
    pub since: Option<String>,
    pub app_bundle_id: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub total: i64,
    pub by_status: HashMap<String, i64>,
    pub by_device: Vec<(String, i64)>,
    pub by_os: Vec<(String, i64)>,
    pub unfixed: i64,
}

// ─── Database implementation ─────────────────────────────────────────────────

const CRASH_SELECT: &str = "
    SELECT c.id, c.app_id, c.submission_id, c.created_at, c.synced_at,
           c.device_model, c.os_version, c.app_platform, c.architecture,
           c.tester_email, c.tester_comment, c.bundle_id, c.build_id,
           c.app_uptime_ms, c.battery_pct, c.connection_type,
           c.has_log, c.log_path, c.status, c.fixed_at, c.fix_notes,
           c.duplicate_of, a.bundle_id, a.name
    FROM crashes c
    JOIN apps a ON a.id = c.app_id
";

impl CrashDb {
    pub fn open(path: &Path) -> Result<Self> {
        let conn =
            Connection::open(path).with_context(|| format!("open db: {}", path.display()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS apps (
                id        INTEGER PRIMARY KEY AUTOINCREMENT,
                asc_id    TEXT UNIQUE,
                bundle_id TEXT UNIQUE NOT NULL,
                name      TEXT
            );

            CREATE TABLE IF NOT EXISTS crashes (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                app_id          INTEGER NOT NULL REFERENCES apps(id),
                submission_id   TEXT UNIQUE NOT NULL,
                created_at      TEXT NOT NULL,
                synced_at       TEXT NOT NULL DEFAULT (datetime('now')),
                device_model    TEXT,
                os_version      TEXT,
                app_platform    TEXT,
                architecture    TEXT,
                tester_email    TEXT,
                tester_comment  TEXT,
                bundle_id       TEXT,
                build_id        TEXT,
                app_uptime_ms   INTEGER,
                battery_pct     INTEGER,
                connection_type TEXT,
                has_log         INTEGER DEFAULT 0,
                log_path        TEXT,
                status          TEXT DEFAULT 'new'
                                CHECK(status IN ('new','investigating','fixed','wontfix','duplicate')),
                fixed_at        TEXT,
                fix_notes       TEXT,
                duplicate_of    INTEGER REFERENCES crashes(id)
            );

            CREATE INDEX IF NOT EXISTS idx_crashes_status     ON crashes(status);
            CREATE INDEX IF NOT EXISTS idx_crashes_created     ON crashes(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_crashes_submission   ON crashes(submission_id);
            CREATE INDEX IF NOT EXISTS idx_crashes_app          ON crashes(app_id);
            ",
        )?;
        Ok(())
    }

    // ─── Apps ────────────────────────────────────────────────────────────

    /// Insert or update an app, returning its local DB id.
    pub fn upsert_app(
        &self,
        bundle_id: &str,
        asc_id: Option<&str>,
        name: Option<&str>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO apps (bundle_id, asc_id, name) VALUES (?1, ?2, ?3)
             ON CONFLICT(bundle_id) DO UPDATE SET
               asc_id = COALESCE(?2, asc_id),
               name   = COALESCE(?3, name)",
            params![bundle_id, asc_id, name],
        )?;

        let id: i64 = self.conn.query_row(
            "SELECT id FROM apps WHERE bundle_id = ?1",
            params![bundle_id],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    // ─── Crashes ─────────────────────────────────────────────────────────

    /// Insert a new crash. Returns the local id, or None if it already exists.
    pub fn insert_crash(&self, c: &NewCrash) -> Result<Option<i64>> {
        let affected = self.conn.execute(
            "INSERT OR IGNORE INTO crashes
             (app_id, submission_id, created_at, device_model, os_version,
              app_platform, architecture, tester_email, tester_comment,
              bundle_id, build_id, app_uptime_ms, battery_pct, connection_type)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                c.app_id,
                c.submission_id,
                c.created_at,
                c.device_model,
                c.os_version,
                c.app_platform,
                c.architecture,
                c.tester_email,
                c.tester_comment,
                c.bundle_id,
                c.build_id,
                c.app_uptime_ms,
                c.battery_pct,
                c.connection_type,
            ],
        )?;

        if affected == 0 {
            return Ok(None); // already exists
        }
        Ok(Some(self.conn.last_insert_rowid()))
    }

    pub fn get_crash(&self, id: i64) -> Result<Option<CrashRow>> {
        let sql = format!("{CRASH_SELECT} WHERE c.id = ?1");
        self.conn
            .query_row(&sql, params![id], row_to_crash)
            .optional()
            .context("get crash")
    }

    pub fn list_crashes(&self, f: &CrashFilters) -> Result<Vec<CrashRow>> {
        let mut conditions = Vec::new();
        let mut bind_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(ref statuses) = f.status {
            let placeholders: Vec<String> = statuses
                .iter()
                .map(|_| {
                    let p = format!("?{idx}");
                    idx += 1;
                    p
                })
                .collect();
            conditions.push(format!("c.status IN ({})", placeholders.join(",")));
            for s in statuses {
                bind_values.push(Box::new(s.clone()));
            }
        }

        if let Some(ref since) = f.since {
            conditions.push(format!("c.created_at >= ?{idx}"));
            bind_values.push(Box::new(since.clone()));
            idx += 1;
        }

        if let Some(ref bundle) = f.app_bundle_id {
            conditions.push(format!("a.bundle_id = ?{idx}"));
            bind_values.push(Box::new(bundle.clone()));
            idx += 1;
        }

        let _ = idx; // suppress unused warning

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "{CRASH_SELECT}{where_clause} ORDER BY c.created_at DESC LIMIT ?{}",
            bind_values.len() + 1
        );
        bind_values.push(Box::new(f.limit as i64));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            bind_values.iter().map(|b| b.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_ref.as_slice(), row_to_crash)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Return crashes that don't yet have a downloaded log.
    pub fn crashes_missing_logs(&self) -> Result<Vec<CrashRow>> {
        let sql = format!("{CRASH_SELECT} WHERE c.has_log = 0 ORDER BY c.created_at DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], row_to_crash)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn set_log(&self, id: i64, log_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE crashes SET has_log = 1, log_path = ?1 WHERE id = ?2",
            params![log_path, id],
        )?;
        Ok(())
    }

    pub fn update_status(&self, id: i64, status: &str, notes: Option<&str>) -> Result<bool> {
        let fixed_at = if status == "fixed" {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };
        let affected = self.conn.execute(
            "UPDATE crashes SET status = ?1, fix_notes = COALESCE(?2, fix_notes),
             fixed_at = COALESCE(?3, fixed_at) WHERE id = ?4",
            params![status, notes, fixed_at, id],
        )?;
        Ok(affected > 0)
    }

    pub fn mark_duplicate(&self, id: i64, of_id: i64) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE crashes SET status = 'duplicate', duplicate_of = ?1 WHERE id = ?2",
            params![of_id, id],
        )?;
        Ok(affected > 0)
    }

    pub fn reopen(&self, id: i64) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE crashes SET status = 'new', fixed_at = NULL, fix_notes = NULL, \
             duplicate_of = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(affected > 0)
    }

    pub fn stats(&self, app_bundle_id: Option<&str>) -> Result<Stats> {
        let filter = if let Some(b) = app_bundle_id {
            format!(" WHERE a.bundle_id = '{}'", b.replace('\'', "''"))
        } else {
            String::new()
        };

        let total: i64 = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM crashes c JOIN apps a ON a.id = c.app_id{filter}"),
            [],
            |r| r.get(0),
        )?;

        let mut by_status = HashMap::new();
        {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT c.status, COUNT(*) FROM crashes c \
                 JOIN apps a ON a.id = c.app_id{filter} GROUP BY c.status"
            ))?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in rows {
                let (s, n) = r?;
                by_status.insert(s, n);
            }
        }

        let unfixed = total
            - by_status.get("fixed").copied().unwrap_or(0)
            - by_status.get("wontfix").copied().unwrap_or(0)
            - by_status.get("duplicate").copied().unwrap_or(0);

        let by_device = self.top_n_group(&format!(
            "SELECT c.device_model, COUNT(*) FROM crashes c \
             JOIN apps a ON a.id = c.app_id{filter} \
             WHERE c.device_model IS NOT NULL \
             GROUP BY c.device_model ORDER BY COUNT(*) DESC LIMIT 15"
        ))?;

        let by_os = self.top_n_group(&format!(
            "SELECT c.os_version, COUNT(*) FROM crashes c \
             JOIN apps a ON a.id = c.app_id{filter} \
             WHERE c.os_version IS NOT NULL \
             GROUP BY c.os_version ORDER BY COUNT(*) DESC LIMIT 15"
        ))?;

        Ok(Stats {
            total,
            by_status,
            by_device,
            by_os,
            unfixed,
        })
    }

    fn top_n_group(&self, sql: &str) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn count_unfixed(&self) -> Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM crashes WHERE status IN ('new','investigating')",
                [],
                |r| r.get(0),
            )
            .context("count unfixed")
    }

    pub fn count_total(&self) -> Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM crashes", [], |r| r.get(0))
            .context("count total")
    }
}

fn row_to_crash(row: &rusqlite::Row) -> rusqlite::Result<CrashRow> {
    Ok(CrashRow {
        id: row.get(0)?,
        app_id: row.get(1)?,
        submission_id: row.get(2)?,
        created_at: row.get(3)?,
        synced_at: row.get(4)?,
        device_model: row.get(5)?,
        os_version: row.get(6)?,
        app_platform: row.get(7)?,
        architecture: row.get(8)?,
        tester_email: row.get(9)?,
        tester_comment: row.get(10)?,
        bundle_id: row.get(11)?,
        build_id: row.get(12)?,
        app_uptime_ms: row.get(13)?,
        battery_pct: row.get(14)?,
        connection_type: row.get(15)?,
        has_log: row.get::<_, i32>(16)? != 0,
        log_path: row.get(17)?,
        status: row.get(18)?,
        fixed_at: row.get(19)?,
        fix_notes: row.get(20)?,
        duplicate_of: row.get(21)?,
        app_bundle_id: row.get(22)?,
        app_name: row.get(23)?,
    })
}
