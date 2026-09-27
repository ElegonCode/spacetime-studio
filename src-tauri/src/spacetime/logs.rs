//! Streams `/logs?follow=true` to the webview over a Tauri channel, so the
//! logs page receives new lines as the module writes them instead of
//! re-downloading the last N lines on a timer.

use reqwest::Client;
use serde::Serialize;
use tauri::ipc::Channel;

use super::api::{describe_status, Api, REQUEST_TIMEOUT};
use super::profiles::ConnectionProfile;

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LogEvent {
    /// Raw newline-delimited JSON log records, parsed by the webview.
    Lines { lines: Vec<String> },
    Error { message: String },
    /// The server closed the stream (always the case when not following).
    End,
}

pub async fn stream(
    client: Client,
    profile: ConnectionProfile,
    num_lines: u32,
    follow: bool,
    channel: Channel<LogEvent>,
) {
    let api = Api::new(&client, &profile);
    let path = api.database_path(&format!("/logs?num_lines={num_lines}&follow={follow}"));
    let mut request = api.get(&path);
    if !follow {
        request = request.timeout(REQUEST_TIMEOUT);
    }

    log::info!("Opening log stream for {} (follow={follow})", profile.database);

    let mut response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            log::warn!("Log request failed: {error}");
            let _ = channel.send(LogEvent::Error {
                message: format!("Log request failed: {error}"),
            });
            return;
        }
    };

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::warn!("Log request returned {status}");
        let _ = channel.send(LogEvent::Error {
            message: describe_status("Log request", status, &body),
        });
        return;
    }

    // Buffer raw bytes so a UTF-8 character split across chunks is not mangled.
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                buffer.extend_from_slice(&chunk);
                let lines = drain_complete_lines(&mut buffer);
                if !lines.is_empty() && channel.send(LogEvent::Lines { lines }).is_err() {
                    log::info!("Log stream receiver went away; closing");
                    return;
                }
            }
            Ok(None) => break,
            Err(error) => {
                log::warn!("Log stream interrupted: {error}");
                let _ = channel.send(LogEvent::Error {
                    message: format!("Log stream interrupted: {error}"),
                });
                return;
            }
        }
    }

    let rest = String::from_utf8_lossy(&buffer).trim().to_string();
    if !rest.is_empty() {
        let _ = channel.send(LogEvent::Lines { lines: vec![rest] });
    }
    log::info!("Log stream for {} ended", profile.database);
    let _ = channel.send(LogEvent::End);
}

fn drain_complete_lines(buffer: &mut Vec<u8>) -> Vec<String> {
    let Some(last_newline) = buffer.iter().rposition(|byte| *byte == b'\n') else {
        return Vec::new();
    };
    let complete: Vec<u8> = buffer.drain(..=last_newline).collect();
    String::from_utf8_lossy(&complete)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_complete_lines_are_drained() {
        let mut buffer = b"{\"a\":1}\n{\"b\":2}\n{\"c\"".to_vec();
        assert_eq!(drain_complete_lines(&mut buffer), vec!["{\"a\":1}", "{\"b\":2}"]);
        assert_eq!(buffer, b"{\"c\"");
        buffer.extend_from_slice(b":3}\n");
        assert_eq!(drain_complete_lines(&mut buffer), vec!["{\"c\":3}"]);
        assert!(buffer.is_empty());
    }
}
