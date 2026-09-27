//! Thin client for the SpacetimeDB HTTP API. Every request for a profile goes
//! through [`Api`] so auth, timeouts, error messages and logging stay uniform.

use reqwest::{header::HeaderMap, Client, RequestBuilder, StatusCode};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

use super::profiles::{get_token, ConnectionProfile};
use super::schema::{product_columns, ColumnSummary, SchemaSummary, TypeContext};
use super::sql::js_safe_value;

/// Applied to every request except the follow-mode log stream, which stays open.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SCHEMA_VERSION: u16 = 9;

/// One shared client so connections and TLS sessions are reused across calls.
pub fn build_client() -> Client {
    Client::builder()
        .user_agent(concat!("spacetime-studio/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_else(|error| {
            log::error!("Could not configure HTTP client, using defaults: {error}");
            Client::new()
        })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlStatementResult {
    pub columns: Vec<ColumnSummary>,
    pub rows: Vec<Vec<Value>>,
    pub duration_micros: Option<u64>,
}

pub struct Api<'a> {
    client: &'a Client,
    profile: &'a ConnectionProfile,
    token: Option<String>,
}

impl<'a> Api<'a> {
    /// Uses the token stored in the OS keychain for this profile.
    pub fn new(client: &'a Client, profile: &'a ConnectionProfile) -> Self {
        let token = get_token(&profile.id);
        Self::with_token(client, profile, token)
    }

    pub fn with_token(
        client: &'a Client,
        profile: &'a ConnectionProfile,
        token: Option<String>,
    ) -> Self {
        Self {
            client,
            profile,
            token: token.filter(|token| !token.trim().is_empty()),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.profile.base_url, path)
    }

    pub fn database_path(&self, suffix: &str) -> String {
        format!("/v1/database/{}{}", self.profile.database, suffix)
    }

    fn authorize(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    pub fn get(&self, path: &str) -> RequestBuilder {
        self.authorize(self.client.get(self.url(path)))
    }

    pub fn post(&self, path: &str) -> RequestBuilder {
        self.authorize(self.client.post(self.url(path)))
    }

    /// Sends `request` and returns the body of a successful response.
    pub async fn send(what: &str, request: RequestBuilder) -> Result<(String, HeaderMap), String> {
        Self::send_within(what, request, REQUEST_TIMEOUT).await
    }

    async fn send_within(
        what: &str,
        request: RequestBuilder,
        timeout: Duration,
    ) -> Result<(String, HeaderMap), String> {
        let response = request.timeout(timeout).send().await.map_err(|error| {
            log::warn!("{what} failed: {error}");
            describe_transport_error(what, &error)
        })?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .text()
            .await
            .map_err(|error| format!("Could not read the {what} response: {error}"))?;

        if !status.is_success() {
            log::warn!("{what} returned {status}: {}", truncate(&body, 500));
            return Err(describe_status(what, status, &body));
        }

        Ok((body, headers))
    }

    pub async fn get_json(&self, what: &str, path: &str) -> Result<(Value, HeaderMap), String> {
        let (body, headers) = Self::send(what, self.get(path)).await?;
        let value = if body.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&body).map_err(|error| {
                format!("Invalid JSON in the {what} response: {error}; body: {}", truncate(&body, 300))
            })?
        };
        Ok((value, headers))
    }

    pub async fn ping(&self) -> Result<(), String> {
        // Short timeout: the connection picker pings every saved host at once.
        Self::send_within("Ping", self.client.get(self.url("/v1/ping")), Duration::from_secs(5))
            .await?;
        Ok(())
    }

    pub async fn schema(&self) -> Result<SchemaSummary, String> {
        let path = self.database_path(&format!("/schema?version={SCHEMA_VERSION}"));
        let (raw, _) = self.get_json("Schema request", &path).await?;
        Ok(super::schema::summarize_schema(raw))
    }

    pub async fn sql(&self, sql: &str) -> Result<Vec<SqlStatementResult>, String> {
        log::info!("SQL on {}: {}", self.profile.database, truncate(sql, 300));
        let request = self
            .post(&self.database_path("/sql"))
            .header(reqwest::header::CONTENT_TYPE, "text/plain")
            .body(sql.to_string());
        let (body, _) = Self::send("SQL request", request).await?;
        let value: Value = serde_json::from_str(&body)
            .map_err(|error| format!("Invalid SQL JSON response: {error}; body: {}", truncate(&body, 300)))?;
        parse_sql_response(&value)
    }

    pub async fn call_reducer(&self, name: &str, args: &Value) -> Result<Value, String> {
        log::info!("Calling reducer '{name}' on {}", self.profile.database);
        let request = self
            .post(&self.database_path(&format!("/call/{name}")))
            .json(args);
        let (body, _) = Self::send("Reducer call", request).await?;
        if body.trim().is_empty() {
            Ok(Value::Null)
        } else {
            Ok(serde_json::from_str(&body).unwrap_or(Value::String(body)))
        }
    }
}

pub fn parse_sql_response(value: &Value) -> Result<Vec<SqlStatementResult>, String> {
    let statements = value
        .as_array()
        .ok_or_else(|| "Unexpected SQL response: expected an array of result sets".to_string())?;
    let context = TypeContext::inline();

    Ok(statements
        .iter()
        .map(|statement| {
            let elements = statement
                .pointer("/schema/elements")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let rows = statement
                .get("rows")
                .and_then(Value::as_array)
                .map(|rows| {
                    rows.iter()
                        .map(|row| match row {
                            Value::Array(values) => {
                                values.iter().cloned().map(js_safe_value).collect()
                            }
                            other => vec![other.clone()],
                        })
                        .collect()
                })
                .unwrap_or_default();

            SqlStatementResult {
                columns: product_columns(&context, elements, "column"),
                rows,
                duration_micros: statement
                    .get("total_duration_micros")
                    .and_then(Value::as_u64),
            }
        })
        .collect())
}

pub fn truncate(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((index, _)) => format!("{}…", &text[..index]),
        None => text.to_string(),
    }
}

fn describe_transport_error(what: &str, error: &reqwest::Error) -> String {
    if error.is_timeout() {
        format!("{what} timed out. The host may be down or overloaded.")
    } else if error.is_connect() {
        format!("Could not reach the SpacetimeDB host ({what}): {error}")
    } else {
        format!("{what} failed: {error}")
    }
}

pub fn describe_status(what: &str, status: StatusCode, body: &str) -> String {
    let body = truncate(body.trim(), 500);
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => format!(
            "{what} was rejected ({status}). The token on this connection is missing, expired, or lacks access. {body}"
        ),
        StatusCode::NOT_FOUND => format!("{what} returned 404: the database or endpoint was not found. {body}"),
        _ => format!("{what} returned {status}: {body}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sql_results_use_the_response_column_names() {
        let response = json!([{
            "schema": { "elements": [
                { "name": { "some": "id" }, "algebraic_type": { "U64": [] } },
                { "name": { "some": "n" }, "algebraic_type": { "String": [] } }
            ] },
            "rows": [[1, "a"], [18446744073709551615u64, "b"]],
            "total_duration_micros": 120
        }]);

        let parsed = parse_sql_response(&response).unwrap();
        assert_eq!(parsed.len(), 1);
        let names: Vec<_> = parsed[0].columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["id", "n"]);
        assert_eq!(parsed[0].rows[1][0], json!("18446744073709551615"));
        assert_eq!(parsed[0].duration_micros, Some(120));
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        assert_eq!(truncate("héllo", 2), "hé…");
        assert_eq!(truncate("hi", 5), "hi");
    }
}
