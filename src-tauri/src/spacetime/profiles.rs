use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

const KEYRING_SERVICE: &str = "spacetime-studio";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub database: String,
    pub identity: Option<String>,
    pub has_token: bool,
    /// Blocks every write (row edits, reducer calls, mutating SQL) in the backend,
    /// so a production database can be browsed without the risk of changing it.
    /// Profiles saved before this flag existed stay writable.
    #[serde(default)]
    pub read_only: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

impl ConnectionProfile {
    pub fn ensure_writable(&self) -> Result<(), String> {
        if self.read_only {
            log::warn!("Blocked a write on read-only connection '{}'", self.name);
            Err(format!(
                "'{}' is in read-only mode. Turn it off in the connection settings to make changes.",
                self.name
            ))
        } else {
            Ok(())
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn profiles_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate app data directory: {error}"))?;
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create app data directory: {error}"))?;
    Ok(dir.join("connections.json"))
}

pub fn load_profiles(app: &AppHandle) -> Vec<ConnectionProfile> {
    let path = match profiles_path(app) {
        Ok(path) => path,
        Err(error) => {
            log::error!("{error}");
            return Vec::new();
        }
    };

    let Ok(contents) = fs::read_to_string(&path) else {
        log::info!("No saved connections at {}", path.display());
        return Vec::new();
    };

    match serde_json::from_str::<Vec<ConnectionProfile>>(&contents) {
        Ok(profiles) => {
            log::info!("Loaded {} saved connection(s)", profiles.len());
            profiles
        }
        Err(error) => {
            log::error!("Could not parse {}: {error}", path.display());
            Vec::new()
        }
    }
}

pub fn persist_profiles(app: &AppHandle, profiles: &[ConnectionProfile]) -> Result<(), String> {
    let path = profiles_path(app)?;
    let json = serde_json::to_string_pretty(profiles)
        .map_err(|error| format!("Could not serialize connections: {error}"))?;
    fs::write(path, json).map_err(|error| format!("Could not store connections: {error}"))
}

pub fn normalize_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');

    if trimmed.is_empty() || trimmed.contains("://") {
        return trimmed.to_string();
    }

    // Hosting providers hand out bare domains such as `my-app.up.railway.app`.
    // reqwest rejects those, so assume the scheme the host most likely serves.
    let authority = trimmed.split('/').next().unwrap_or(trimmed);
    let host = authority
        .rsplit_once(':')
        .map(|(host, _port)| host)
        .unwrap_or(authority);
    let scheme = if matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
        "http"
    } else {
        "https"
    };

    format!("{scheme}://{trimmed}")
}

fn token_key(connection_id: &str) -> String {
    format!("connection:{connection_id}")
}

pub fn set_token(connection_id: &str, token: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, &token_key(connection_id))
        .map_err(|error| format!("Could not open OS credential store: {error}"))?
        .set_password(token)
        .map_err(|error| format!("Could not store token securely: {error}"))
}

pub fn get_token(connection_id: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, &token_key(connection_id))
        .ok()
        .and_then(|entry| entry.get_password().ok())
}

pub fn delete_token(connection_id: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &token_key(connection_id)) {
        let _ = entry.delete_credential();
    }
}

/// Builds an id that cannot collide with a profile that already exists. A bare
/// millisecond timestamp collides when two saves land in the same millisecond, and a
/// collision silently overwrites the earlier profile instead of adding a new one.
pub fn new_connection_id(existing: &[ConnectionProfile], now: u64) -> String {
    let mut candidate = format!("conn-{now}");
    let mut suffix = 1u32;

    while existing.iter().any(|profile| profile.id == candidate) {
        candidate = format!("conn-{now}-{suffix}");
        suffix += 1;
    }

    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with_id(id: &str) -> ConnectionProfile {
        ConnectionProfile {
            id: id.to_string(),
            name: "Existing".to_string(),
            base_url: "https://maincloud.spacetimedb.com".to_string(),
            database: "elegon".to_string(),
            identity: None,
            has_token: false,
            read_only: false,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn new_connection_id_uses_the_timestamp_when_it_is_free() {
        assert_eq!(new_connection_id(&[], 42), "conn-42");
    }

    #[test]
    fn new_connection_id_never_reuses_an_existing_id() {
        let existing = vec![profile_with_id("conn-42"), profile_with_id("conn-42-1")];
        let id = new_connection_id(&existing, 42);

        assert_eq!(id, "conn-42-2");
        assert!(!existing.iter().any(|profile| profile.id == id));
    }

    #[test]
    fn profiles_saved_before_read_only_existed_stay_writable() {
        let json = r#"{"id":"a","name":"n","baseUrl":"http://x","database":"d","identity":null,"hasToken":false,"createdAt":1,"updatedAt":1}"#;
        let profile: ConnectionProfile = serde_json::from_str(json).unwrap();
        assert!(!profile.read_only);
        assert!(profile.ensure_writable().is_ok());
    }

    #[test]
    fn read_only_profiles_reject_writes() {
        let mut profile = profile_with_id("a");
        profile.read_only = true;
        assert!(profile.ensure_writable().is_err());
    }

    #[test]
    fn normalize_url_keeps_an_explicit_scheme() {
        assert_eq!(
            normalize_url("  http://localhost:3000/ "),
            "http://localhost:3000"
        );
        assert_eq!(
            normalize_url("https://maincloud.spacetimedb.com"),
            "https://maincloud.spacetimedb.com"
        );
    }

    #[test]
    fn normalize_url_assumes_https_for_a_bare_remote_host() {
        assert_eq!(
            normalize_url("my-app.up.railway.app"),
            "https://my-app.up.railway.app"
        );
        assert_eq!(
            normalize_url("my-app.up.railway.app:8080/"),
            "https://my-app.up.railway.app:8080"
        );
    }

    #[test]
    fn normalize_url_assumes_http_for_a_bare_loopback_host() {
        assert_eq!(normalize_url("localhost:3000"), "http://localhost:3000");
        assert_eq!(normalize_url("127.0.0.1"), "http://127.0.0.1");
        assert_eq!(normalize_url("[::1]:3000"), "http://[::1]:3000");
    }

    #[test]
    fn normalize_url_leaves_an_empty_value_empty() {
        assert_eq!(normalize_url("   "), "");
    }
}
