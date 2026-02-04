//! Configuration and data-directory resolution.
//!
//! Resolution order:
//!   1. `--data-dir PATH`  (explicit override)
//!   2. `./asc-crashes/`   (project-local, if it exists)
//!   3. `~/.asc-crashes/`  (global default)

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Top-level config from `config.toml`.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub api: ApiConfig,
    #[serde(default)]
    pub apps: Vec<AppEntry>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiConfig {
    pub issuer_id: String,
    pub key_id: String,
    pub private_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppEntry {
    pub bundle_id: String,
    #[allow(dead_code)]
    pub name: Option<String>,
}

impl Config {
    /// Load and validate config from a data directory.
    pub fn load(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("config.toml");
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("could not read {}", path.display()))?;
        let mut cfg: Config = toml::from_str(&contents)
            .with_context(|| format!("invalid TOML in {}", path.display()))?;

        cfg.api.private_key = resolve_key(&cfg.api.private_key, data_dir)?;

        if cfg.apps.is_empty() {
            anyhow::bail!(
                "no [[apps]] entries in {}. Add at least one:\n\n\
                 [[apps]]\n\
                 bundle_id = \"com.example.myapp\"\n",
                path.display()
            );
        }

        Ok(cfg)
    }
}

/// Resolve the data directory.
///
/// Priority: explicit `--data-dir` > `./asc-crashes/` > `~/.asc-crashes/`
pub fn resolve_data_dir(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        return Ok(p.to_path_buf());
    }

    let local = PathBuf::from("asc-crashes");
    if local.join("config.toml").exists() {
        return Ok(std::fs::canonicalize(&local).unwrap_or(local));
    }

    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".asc-crashes"))
}

/// Return the path to `init` into: `./asc-crashes/` or `~/.asc-crashes/`.
pub fn init_data_dir(global: bool) -> Result<PathBuf> {
    if global {
        let home = dirs::home_dir().context("could not determine home directory")?;
        Ok(home.join(".asc-crashes"))
    } else {
        Ok(PathBuf::from("asc-crashes"))
    }
}

/// Resolve a private key value — could be a file path or inline PEM.
fn resolve_key(value: &str, relative_to: &Path) -> Result<String> {
    if value.starts_with("-----BEGIN") {
        return Ok(value.to_string());
    }

    // Expand ~ and resolve relative paths
    let expanded = shellexpand::tilde(value);
    let path = Path::new(expanded.as_ref());
    let path = if path.is_relative() {
        relative_to.join(path)
    } else {
        path.to_path_buf()
    };

    if path.exists() {
        std::fs::read_to_string(&path)
            .with_context(|| format!("could not read key file: {}", path.display()))
    } else {
        anyhow::bail!(
            "private_key '{}' is not a PEM string and file not found at {}",
            value,
            path.display()
        )
    }
}

/// Template config for `init`.
pub const CONFIG_TEMPLATE: &str = r#"# asc-crash-fetcher configuration
#
# API credentials from App Store Connect:
#   https://appstoreconnect.apple.com/access/integrations/api

[api]
issuer_id = "YOUR_ISSUER_ID"
key_id    = "YOUR_KEY_ID"
private_key = "path/to/AuthKey_XXXXXXXX.p8"

# Add one or more apps to monitor for TestFlight crashes.
# Use `asc-crash-fetcher apps` to verify your key works.

[[apps]]
bundle_id = "com.example.myapp"
# name = "My App"  # optional friendly label
"#;
