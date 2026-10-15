//! Starts and stops a standalone SpacetimeDB server (`spacetime start`) owned
//! by the app, and reports on servers already listening on a local port.
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    process::Stdio,
    sync::{Arc, Mutex as StdMutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

use crate::ecosystem::{find_cli, InstallationState};

const MAX_OUTPUT_LINES: usize = 300;

type Output = Arc<StdMutex<VecDeque<String>>>;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerOptions {
    port: u16,
    in_memory: bool,
    // Listen on every interface instead of only this computer.
    allow_remote: bool,
}

struct RunningServer {
    child: Child,
    pid: u32,
    options: ServerOptions,
    started_at: u64,
}

#[derive(Default)]
struct Inner {
    server: Option<RunningServer>,
    // Shared with the reader tasks and kept after exit so the page can show why it stopped.
    output: Output,
    exit_message: Option<String>,
}

#[derive(Default)]
pub struct LocalServerState(Mutex<Inner>);

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ServerState {
    Stopped,
    Starting,
    Running,
    // Something not started by the app answers on the requested port.
    External,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalServerStatus {
    state: ServerState,
    pid: Option<u32>,
    options: Option<ServerOptions>,
    url: Option<String>,
    started_at: Option<u64>,
    exit_message: Option<String>,
    output: Vec<String>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

fn local_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

async fn responds_to_ping(port: u16) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
    else {
        return false;
    };
    client
        .get(format!("{}/v1/ping", local_url(port)))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

fn port_in_use(addr: SocketAddr) -> bool {
    // Binding a specific address can succeed beside a wildcard listener on
    // Windows, so also check whether anything accepts loopback connections.
    let loopback = SocketAddr::from((Ipv4Addr::LOCALHOST, addr.port()));
    TcpListener::bind(addr).is_err()
        || TcpStream::connect_timeout(&loopback, Duration::from_millis(300)).is_ok()
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip a CSI sequence: ESC [ parameters final-byte.
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn push_line(output: &Output, line: String) {
    let mut output = output.lock().unwrap_or_else(|e| e.into_inner());
    if output.len() == MAX_OUTPUT_LINES {
        output.pop_front();
    }
    output.push_back(line);
}

fn forward_output(stream: impl AsyncRead + Unpin + Send + 'static, output: Output) {
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            push_line(&output, strip_ansi(&line));
        }
    });
}

impl Inner {
    /// Notices a server that exited on its own.
    fn reap(&mut self) {
        let Some(server) = self.server.as_mut() else {
            return;
        };
        match server.child.try_wait() {
            Ok(None) => {}
            Ok(Some(status)) => {
                self.exit_message = Some(format!("The server exited unexpectedly ({status})."));
                self.server = None;
            }
            Err(error) => {
                self.exit_message = Some(format!("Lost track of the server process: {error}"));
                self.server = None;
            }
        }
    }

    async fn status(&mut self, probe_port: Option<u16>) -> LocalServerStatus {
        self.reap();
        let output = self
            .output
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect();
        if let Some(server) = &self.server {
            let port = server.options.port;
            return LocalServerStatus {
                state: if responds_to_ping(port).await {
                    ServerState::Running
                } else {
                    ServerState::Starting
                },
                pid: Some(server.pid),
                options: Some(server.options),
                url: Some(local_url(port)),
                started_at: Some(server.started_at),
                exit_message: None,
                output,
            };
        }
        let external = match probe_port {
            Some(port) => responds_to_ping(port).await,
            None => false,
        };
        LocalServerStatus {
            state: if external {
                ServerState::External
            } else {
                ServerState::Stopped
            },
            pid: None,
            options: None,
            url: probe_port.filter(|_| external).map(local_url),
            started_at: None,
            exit_message: self.exit_message.clone(),
            output,
        }
    }

    async fn stop(&mut self) -> Result<(), String> {
        let Some(mut server) = self.server.take() else {
            return Ok(());
        };
        // `spacetime start` runs the server binary as a child process, so the
        // whole process tree has to be stopped, not just the launcher.
        #[cfg(windows)]
        {
            let mut taskkill = Command::new("taskkill");
            taskkill
                .args(["/PID", &server.pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x08000000); // CREATE_NO_WINDOW
            let _ = taskkill.status().await;
        }
        #[cfg(unix)]
        {
            // The server leads its own process group; ask it to shut down cleanly first.
            let group = format!("-{}", server.pid);
            let _ = Command::new("kill")
                .args(["-TERM", "--", &group])
                .status()
                .await;
            if tokio::time::timeout(Duration::from_secs(10), server.child.wait())
                .await
                .is_err()
            {
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &group])
                    .status()
                    .await;
            }
        }
        if tokio::time::timeout(Duration::from_secs(5), server.child.wait())
            .await
            .is_err()
        {
            let _ = server.child.kill().await;
        }
        if server.child.try_wait().ok().flatten().is_none() {
            let message = format!("Could not stop the server process {}.", server.pid);
            self.server = Some(server);
            return Err(message);
        }
        push_line(
            &self.output,
            "— Server stopped by Spacetime Studio —".into(),
        );
        self.exit_message = None;
        Ok(())
    }
}

fn validate_port(port: u16) -> Result<(), String> {
    if port < 1024 {
        return Err("Choose a port between 1024 and 65535.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_local_server_status(
    state: tauri::State<'_, LocalServerState>,
    probe_port: Option<u16>,
) -> Result<LocalServerStatus, String> {
    Ok(state.0.lock().await.status(probe_port).await)
}

#[tauri::command]
pub async fn start_local_server(
    state: tauri::State<'_, LocalServerState>,
    installations: tauri::State<'_, InstallationState>,
    options: ServerOptions,
) -> Result<LocalServerStatus, String> {
    validate_port(options.port)?;
    let mut inner = state.0.lock().await;
    // Never launch binaries the version manager is in the middle of replacing.
    let _install_guard = installations
        .0
        .try_lock()
        .map_err(|_| "Wait for the SpacetimeDB installation operation to finish.")?;
    inner.start(options).await?;
    Ok(inner.status(None).await)
}

impl Inner {
    async fn start(&mut self, options: ServerOptions) -> Result<(), String> {
        self.reap();
        if self.server.is_some() {
            return Err("The local server is already running.".into());
        }
        let cli =
            find_cli().ok_or("SpacetimeDB is not installed. Install the official CLI first.")?;
        let ip = if options.allow_remote {
            Ipv4Addr::UNSPECIFIED
        } else {
            Ipv4Addr::LOCALHOST
        };
        let addr = SocketAddr::from((ip, options.port));
        if port_in_use(addr) {
            return Err(if responds_to_ping(options.port).await {
                format!(
                    "A SpacetimeDB server is already running on port {}.",
                    options.port
                )
            } else {
                format!(
                    "Port {} is already in use by another program.",
                    options.port
                )
            });
        }

        let mut command = Command::new(&cli);
        command
            .args(["start", "--listen-addr", &addr.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0");
        if options.in_memory {
            command.arg("--in-memory");
        }
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        #[cfg(unix)]
        command.process_group(0);

        self.output = Output::default();
        let mut child = command
            .spawn()
            .map_err(|e| format!("Could not start {}: {e}", cli.display()))?;
        let pid = child.id().ok_or("The server exited immediately.")?;
        if let Some(stdout) = child.stdout.take() {
            forward_output(stdout, self.output.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            forward_output(stderr, self.output.clone());
        }
        log::info!("Started local SpacetimeDB server (pid {pid}) on {addr}");
        self.exit_message = None;
        self.server = Some(RunningServer {
            child,
            pid,
            options,
            started_at: now_ms(),
        });
        Ok(())
    }
}

#[tauri::command]
pub async fn stop_local_server(
    state: tauri::State<'_, LocalServerState>,
) -> Result<LocalServerStatus, String> {
    let mut inner = state.0.lock().await;
    inner.stop().await?;
    log::info!("Stopped local SpacetimeDB server");
    Ok(inner.status(None).await)
}

/// Stops an owned server when the app quits so it does not outlive the window.
pub fn shutdown(state: &LocalServerState) {
    tauri::async_runtime::block_on(async {
        if let Err(error) = state.0.lock().await.stop().await {
            log::warn!("{error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_terminal_colours() {
        assert_eq!(
            strip_ansi("\u{1b}[2m2026\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m started"),
            "2026  INFO started"
        );
        assert_eq!(strip_ansi("plain — text"), "plain — text");
    }

    #[test]
    fn keeps_only_recent_output() {
        let output = Output::default();
        for i in 0..MAX_OUTPUT_LINES + 5 {
            push_line(&output, i.to_string());
        }
        let output = output.lock().unwrap();
        assert_eq!(output.len(), MAX_OUTPUT_LINES);
        assert_eq!(output.front().unwrap(), "5");
    }

    #[tokio::test]
    #[ignore = "starts and stops a real in-memory server with this computer's CLI"]
    async fn starts_and_stops_a_live_server() {
        let options = ServerOptions {
            port: 3917,
            in_memory: true,
            allow_remote: false,
        };
        let mut inner = Inner::default();
        inner.start(options).await.unwrap();
        assert!(
            inner.start(options).await.is_err(),
            "second start must fail"
        );
        let mut ready = false;
        for _ in 0..60 {
            if inner.status(None).await.state == ServerState::Running {
                ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        let output = inner.status(None).await.output.join("\n");
        assert!(ready, "server never answered /v1/ping:\n{output}");
        inner.stop().await.unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            !responds_to_ping(options.port).await,
            "server tree still running"
        );
        assert!(!port_in_use(SocketAddr::from((
            Ipv4Addr::LOCALHOST,
            options.port
        ))));
        assert_eq!(
            inner.status(Some(options.port)).await.state,
            ServerState::Stopped
        );
    }

    #[test]
    fn rejects_privileged_ports() {
        assert!(validate_port(80).is_err());
        assert!(validate_port(3000).is_ok());
    }
}
