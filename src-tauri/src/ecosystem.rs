//! Official news and release metadata, plus the local CLI's own version manager.
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{process::Command, sync::Mutex};

#[derive(Default)]
pub struct InstallationState(pub(crate) Mutex<()>);

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("Spacetime-Studio")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_spacetime_news() -> Result<String, String> {
    http_client()?
        .get("https://spacetimedb.com/blog/rss.xml")
        .send()
        .await
        .map_err(|e| format!("Could not fetch SpacetimeDB news: {e}"))?
        .error_for_status()
        .map_err(|e| format!("SpacetimeDB news is unavailable: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledVersion {
    version: String,
    current: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalInstallations {
    cli_path: Option<String>,
    current_version: Option<String>,
    versions: Vec<InstalledVersion>,
    // A plain CLI without the version manager is detected, but cannot be managed.
    management_error: Option<String>,
}

pub(crate) fn find_cli() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "spacetime.exe"
    } else {
        "spacetime"
    };
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        // Never resolve from an empty PATH entry (the app's working directory).
        candidates.extend(
            std::env::split_paths(&path)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(name)),
        );
    }
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local).join("SpacetimeDB").join(name));
        }
    } else if let Some(home) = std::env::var_os("HOME") {
        // Desktop apps on macOS/Linux can inherit a smaller PATH than a shell.
        candidates.push(PathBuf::from(home).join(".local/bin").join(name));
    }
    candidates.into_iter().find(|p| p.is_file())
}

async fn run_cli(path: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut command = Command::new(path);
    command
        .args(args)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0");
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| {
            format!(
                "spacetime {} timed out. Refresh to check the installation before retrying.",
                args.join(" ")
            )
        })?
        .map_err(|e| format!("Could not run {}: {e}", path.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(format!(
            "spacetime {} failed ({}).\n{}\n{}",
            args.join(" "),
            output.status,
            stdout,
            stderr
        )
        .trim()
        .to_string());
    }
    Ok([stdout, stderr]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

fn parse_installed(output: &str) -> Result<Vec<InstalledVersion>, String> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let line = line.trim();
            let current = line.ends_with(" (current)");
            let version = line.strip_suffix(" (current)").unwrap_or(line);
            Version::parse(version)
                .map_err(|_| format!("Unrecognised version-manager output: {line}"))?;
            Ok(InstalledVersion {
                version: version.to_owned(),
                current,
            })
        })
        .collect()
}

async fn local_status() -> LocalInstallations {
    let Some(path) = find_cli() else {
        return LocalInstallations {
            cli_path: None,
            current_version: None,
            versions: vec![],
            management_error: None,
        };
    };
    let result = async {
        let output = run_cli(&path, &["version", "list"], Duration::from_secs(20)).await?;
        parse_installed(&output)
    }
    .await;
    match result {
        Ok(versions) => LocalInstallations {
            current_version: versions
                .iter()
                .find(|v| v.current)
                .map(|v| v.version.clone()),
            cli_path: Some(path.display().to_string()),
            versions,
            management_error: None,
        },
        Err(error) => {
            // Still show a version when a manually installed CLI lacks the manager.
            let version_output = run_cli(&path, &["--version"], Duration::from_secs(20))
                .await
                .ok();
            let current_version = version_output.as_deref().and_then(parse_cli_version);
            LocalInstallations {
                cli_path: Some(path.display().to_string()),
                current_version,
                versions: vec![],
                management_error: Some(error),
            }
        }
    }
}

fn parse_cli_version(output: &str) -> Option<String> {
    // The CLI also prints its path, commit and library version. Only read its
    // labelled tool version, never a version-like component from the path.
    output
        .split("spacetimedb tool version ")
        .nth(1)
        .and_then(|s| s.split([';', '\n', ' ']).next())
        .filter(|s| Version::parse(s).is_ok())
        .map(str::to_owned)
}

#[tauri::command]
pub async fn get_local_installations(
    state: tauri::State<'_, InstallationState>,
) -> Result<LocalInstallations, String> {
    let _guard = state
        .0
        .try_lock()
        .map_err(|_| "A SpacetimeDB installation operation is already in progress.")?;
    Ok(local_status().await)
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    published_at: Option<String>,
    prerelease: bool,
    draft: bool,
    assets: Vec<ReleaseAsset>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpacetimeRelease {
    version: String,
    url: String,
    published_at: Option<String>,
    prerelease: bool,
}

fn platform_asset() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("spacetime-x86_64-pc-windows-msvc.zip"),
        ("linux", "x86_64") => Some("spacetime-x86_64-unknown-linux-gnu.tar.gz"),
        ("linux", "aarch64") => Some("spacetime-aarch64-unknown-linux-gnu.tar.gz"),
        ("macos", "x86_64") => Some("spacetime-x86_64-apple-darwin.tar.gz"),
        ("macos", "aarch64") => Some("spacetime-aarch64-apple-darwin.tar.gz"),
        _ => None,
    }
}

#[tauri::command]
pub async fn get_spacetime_releases(page: u32) -> Result<Vec<SpacetimeRelease>, String> {
    if !(1..=100).contains(&page) {
        return Err("Invalid release page.".into());
    }
    let asset_name = platform_asset()
        .ok_or("No official SpacetimeDB binaries are available for this platform.")?;
    let releases: Vec<GithubRelease> = http_client()?
        .get("https://api.github.com/repos/clockworklabs/SpacetimeDB/releases")
        .query(&[("per_page", 100), ("page", page)])
        .send()
        .await
        .map_err(|e| format!("Could not fetch available versions: {e}"))?
        .error_for_status()
        .map_err(|e| {
            format!("Could not fetch available versions (GitHub may be rate limited): {e}")
        })?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let mut releases: Vec<_> = releases
        .into_iter()
        .filter(|r| !r.draft && r.assets.iter().any(|a| a.name == asset_name))
        .filter_map(|r| {
            let version = r
                .tag_name
                .strip_prefix('v')
                .unwrap_or(&r.tag_name)
                .to_owned();
            let parsed = Version::parse(&version).ok()?;
            Some((
                parsed,
                SpacetimeRelease {
                    version,
                    url: r.html_url,
                    published_at: r.published_at,
                    prerelease: r.prerelease,
                },
            ))
        })
        .collect();
    releases.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(releases.into_iter().map(|(_, release)| release).collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VersionAction {
    Install,
    Use,
    Upgrade,
    Uninstall,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionActionResult {
    output: String,
    status: LocalInstallations,
}

fn action_args(action: VersionAction, version: Option<String>) -> Result<Vec<String>, String> {
    let mut args = vec!["version".to_string()];
    match action {
        VersionAction::Upgrade => args.push("upgrade".into()),
        VersionAction::Install | VersionAction::Use | VersionAction::Uninstall => {
            let version = version.ok_or("Select a SpacetimeDB version.")?;
            // Strict semver excludes flags, paths and arbitrary command input.
            Version::parse(&version).map_err(|_| "Invalid SpacetimeDB version.")?;
            args.push(
                if matches!(action, VersionAction::Install) {
                    "install"
                } else if matches!(action, VersionAction::Uninstall) {
                    "uninstall"
                } else {
                    "use"
                }
                .into(),
            );
            args.push(version);
            if matches!(action, VersionAction::Install | VersionAction::Uninstall) {
                args.push("--yes".into());
            }
        }
    }
    Ok(args)
}

fn validate_uninstall(status: &LocalInstallations, version: &str) -> Result<(), String> {
    let installed = status
        .versions
        .iter()
        .find(|v| v.version == version)
        .ok_or_else(|| format!("SpacetimeDB {version} is not installed. Refresh the page."))?;
    if installed.current || status.current_version.as_deref() == Some(version) {
        return Err("Switch to another version before uninstalling the active version.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn manage_spacetime_version(
    state: tauri::State<'_, InstallationState>,
    action: VersionAction,
    version: Option<String>,
) -> Result<VersionActionResult, String> {
    let args = action_args(action, version)?;
    let _guard = state
        .0
        .try_lock()
        .map_err(|_| "A SpacetimeDB installation operation is already in progress.")?;
    let before = local_status().await;
    if args[1] == "uninstall" {
        validate_uninstall(&before, &args[2])?;
    }
    if let Some(error) = before.management_error {
        return Err(error);
    }
    let path = before
        .cli_path
        .ok_or("SpacetimeDB is not installed. Install the official CLI first.")?;
    let arg_refs: Vec<_> = args.iter().map(String::as_str).collect();
    let output = run_cli(Path::new(&path), &arg_refs, Duration::from_secs(15 * 60)).await?;
    let status = local_status().await;
    if let Some(error) = &status.management_error {
        return Err(format!(
            "The command finished but the installation could not be verified.\n{output}\n{error}"
        ));
    }
    match args[1].as_str() {
        "install" if !status.versions.iter().any(|v| v.version == args[2]) =>
            return Err(format!("The command finished but version {} was not found in the installed versions.\n{output}", args[2])),
        "use" if status.current_version.as_deref() != Some(args[2].as_str()) =>
            return Err(format!("The command finished but version {} is not active.\n{output}", args[2])),
        "upgrade" if status.current_version.is_none() =>
            return Err(format!("The update finished but no active version was found.\n{output}")),
        "uninstall" if status.versions.iter().any(|v| v.version == args[2]) =>
            return Err(format!("The command finished but version {} is still installed.\n{output}", args[2])),
        "uninstall" if status.current_version != before.current_version =>
            return Err(format!("The command finished but the active version changed unexpectedly. Refresh to check the installation.\n{output}")),
        _ => {}
    }
    Ok(VersionActionResult { output, status })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_current_and_prerelease_versions() {
        let versions = parse_installed("2.10.1 (current)\r\n2.7.0-hotfix3\n2.0.5\n").unwrap();
        assert_eq!(versions.len(), 3);
        assert!(versions[0].current);
        assert!(!versions[1].current);
        assert_eq!(versions[1].version, "2.7.0-hotfix3");
        assert!(parse_installed("unknown CLI output").is_err());
        assert!(parse_installed("").unwrap().is_empty());
    }

    #[test]
    fn reads_tool_version_without_confusing_path_or_library() {
        assert_eq!(parse_cli_version("spacetime Path: /bin/1.0.0/cli\nspacetimedb tool version 2.10.1; spacetimedb-lib version 2.8.2;"), Some("2.10.1".into()));
        assert_eq!(parse_cli_version("spacetime Path: /bin/1.0.0/cli"), None);
    }

    #[test]
    fn only_allows_structured_version_actions() {
        assert!(action_args(VersionAction::Install, Some("--help".into())).is_err());
        assert!(action_args(VersionAction::Use, Some("../../bin".into())).is_err());
        assert!(action_args(VersionAction::Use, None).is_err());
        assert!(action_args(VersionAction::Uninstall, Some("current".into())).is_err());
        assert!(action_args(VersionAction::Uninstall, Some("../../bin".into())).is_err());
        assert!(action_args(VersionAction::Uninstall, None).is_err());
        assert_eq!(
            action_args(VersionAction::Uninstall, Some("2.8.2".into())).unwrap(),
            ["version", "uninstall", "2.8.2", "--yes"]
        );
        assert_eq!(
            action_args(VersionAction::Install, Some("2.7.0-hotfix3".into())).unwrap(),
            ["version", "install", "2.7.0-hotfix3", "--yes"]
        );
        assert_eq!(
            action_args(VersionAction::Use, Some("2.10.1".into())).unwrap(),
            ["version", "use", "2.10.1"]
        );
        assert_eq!(
            action_args(VersionAction::Upgrade, None).unwrap(),
            ["version", "upgrade"]
        );
    }

    #[test]
    fn uninstall_only_allows_installed_inactive_versions() {
        let mut status = LocalInstallations {
            cli_path: Some("spacetime".into()),
            current_version: Some("2.10.1".into()),
            versions: parse_installed("2.10.1 (current)\n2.8.2").unwrap(),
            management_error: None,
        };
        assert!(validate_uninstall(&status, "2.8.2").is_ok());
        assert!(validate_uninstall(&status, "2.10.1")
            .unwrap_err()
            .contains("active version"));
        assert!(validate_uninstall(&status, "2.0.0")
            .unwrap_err()
            .contains("not installed"));
        // Protect the active version even if the status fields disagree.
        status.versions[0].current = false;
        assert!(validate_uninstall(&status, "2.10.1").is_err());
        status.current_version = None;
        status.versions[0].current = true;
        assert!(validate_uninstall(&status, "2.10.1").is_err());
    }

    #[tokio::test]
    #[ignore = "read-only integration check against this computer's CLI and official feed"]
    async fn reads_live_installation_and_feed() {
        let status = local_status().await;
        assert!(status.cli_path.is_some(), "{status:?}");
        assert!(status.management_error.is_none(), "{status:?}");
        assert!(status.current_version.is_some(), "{status:?}");
        let feed = get_spacetime_news().await.unwrap();
        assert!(feed.contains("<item>") && feed.contains("<dc:creator>"));
        assert!(!get_spacetime_releases(1).await.unwrap().is_empty());
    }
}
