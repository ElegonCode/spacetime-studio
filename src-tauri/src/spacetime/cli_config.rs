//! Reads the `spacetime` CLI's own config so users who have already run
//! `spacetime login` can reuse that token and server list instead of copying
//! the token by hand. The token itself never leaves the backend.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Deserialize)]
struct RawCliConfig {
    default_server: Option<String>,
    spacetimedb_token: Option<String>,
    #[serde(default)]
    server_configs: Vec<RawServerConfig>,
}

#[derive(Deserialize)]
struct RawServerConfig {
    nickname: String,
    host: String,
    protocol: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliServer {
    pub nickname: String,
    pub url: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliConfigSummary {
    pub path: String,
    pub has_token: bool,
    pub servers: Vec<CliServer>,
}

/// `%LocalAppData%\SpacetimeDB\config\cli.toml` on Windows, and
/// `$XDG_CONFIG_HOME/spacetime/cli.toml` (default `~/.config/…`) elsewhere.
fn config_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local).join("SpacetimeDB").join("config").join("cli.toml"));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(xdg).join("spacetime").join("cli.toml"));
    }
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        candidates.push(PathBuf::from(home).join(".config").join("spacetime").join("cli.toml"));
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn load() -> Option<(RawCliConfig, PathBuf)> {
    let path = config_path()?;
    let contents = std::fs::read_to_string(&path)
        .map_err(|error| log::warn!("Could not read {}: {error}", path.display()))
        .ok()?;
    let config = toml::from_str(&contents)
        .map_err(|error| log::warn!("Could not parse {}: {error}", path.display()))
        .ok()?;
    Some((config, path))
}

pub fn summary() -> Option<CliConfigSummary> {
    let (config, path) = load()?;
    let default = config.default_server.as_deref().unwrap_or_default();

    Some(CliConfigSummary {
        path: path.display().to_string(),
        has_token: config
            .spacetimedb_token
            .as_deref()
            .is_some_and(|token| !token.trim().is_empty()),
        servers: config
            .server_configs
            .iter()
            .map(|server| CliServer {
                is_default: server.nickname == default || server.host == default,
                nickname: server.nickname.clone(),
                url: format!("{}://{}", server.protocol, server.host),
            })
            .collect(),
    })
}

pub fn token() -> Result<String, String> {
    load()
        .and_then(|(config, _)| config.spacetimedb_token)
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| "No token found in the spacetime CLI config. Run `spacetime login` first.".into())
}
