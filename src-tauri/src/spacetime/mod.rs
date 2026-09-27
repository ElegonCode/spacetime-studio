//! Tauri commands for talking to SpacetimeDB. The HTTP details live in
//! [`api`], SQL building in [`sql`], schema parsing in [`schema`]; this module
//! only wires them to the webview.

mod api;
mod cli_config;
mod export;
mod logs;
mod profiles;
mod schema;
mod sql;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex,
    },
};
use tauri::{async_runtime::JoinHandle, ipc::Channel, AppHandle};

use api::{Api, SqlStatementResult};
use export::ExportFormat;
use profiles::{now_ms, ConnectionProfile};
use schema::{ColumnSummary, SchemaSummary, TableSummary};

pub use profiles::load_profiles;

/// Upper bound on rows scanned to page through a table. SpacetimeDB SQL has
/// `LIMIT` but no `OFFSET`, so page N is read by fetching the first N pages.
const MAX_BROWSE_ROWS: usize = 10_000;
const MAX_EXPORT_ROWS: usize = 100_000;

pub struct AppState {
    pub profiles: Mutex<Vec<ConnectionProfile>>,
    http: Client,
    log_streams: Mutex<HashMap<u32, JoinHandle<()>>>,
    next_stream_id: AtomicU32,
}

impl AppState {
    pub fn new(profiles: Vec<ConnectionProfile>) -> Self {
        Self {
            profiles: Mutex::new(profiles),
            http: api::build_client(),
            log_streams: Mutex::new(HashMap::new()),
            next_stream_id: AtomicU32::new(1),
        }
    }

    fn profile(&self, connection_id: &str) -> Result<ConnectionProfile, String> {
        self.profiles
            .lock()
            .map_err(|_| "Connection state lock was poisoned".to_string())?
            .iter()
            .find(|profile| profile.id == connection_id)
            .cloned()
            .ok_or_else(|| "Connection profile not found".to_string())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConnectionInput {
    pub id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub database: String,
    pub token: Option<String>,
    /// Copy the token from the `spacetime` CLI config into the keychain.
    #[serde(default)]
    pub use_cli_token: bool,
    pub read_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionInput {
    pub id: Option<String>,
    pub base_url: Option<String>,
    pub database: Option<String>,
    pub token: Option<String>,
    #[serde(default)]
    pub use_cli_token: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionResult {
    pub ok: bool,
    pub identity: Option<String>,
    pub database_identity: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePage {
    pub columns: Vec<ColumnSummary>,
    pub rows: Vec<Value>,
    /// Rows matching the filter, when the server could count them.
    pub total: Option<u64>,
    pub has_more: bool,
    pub page: usize,
    pub page_size: usize,
    /// Set when paging stopped at [`MAX_BROWSE_ROWS`].
    pub scan_limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlResult {
    pub statements: Vec<SqlStatementResult>,
}

fn rows_to_objects(columns: &[ColumnSummary], rows: &[Vec<Value>]) -> Vec<Value> {
    rows.iter()
        .map(|values| {
            Value::Object(
                columns
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        (
                            column.name.clone(),
                            values.get(index).cloned().unwrap_or(Value::Null),
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

fn first_statement(mut statements: Vec<SqlStatementResult>) -> SqlStatementResult {
    if statements.is_empty() {
        SqlStatementResult {
            columns: Vec::new(),
            rows: Vec::new(),
            duration_micros: None,
        }
    } else {
        statements.swap_remove(0)
    }
}

async fn table_summary(api: &Api<'_>, table_name: &str) -> Result<TableSummary, String> {
    api.schema()
        .await?
        .tables
        .into_iter()
        .find(|table| table.name == table_name)
        .ok_or_else(|| format!("Table '{table_name}' was not found in the schema"))
}

async fn count_rows(api: &Api<'_>, table_name: &str, where_sql: &str) -> Result<u64, String> {
    let sql = format!(
        "SELECT COUNT(*) AS n FROM {}{where_sql};",
        sql::quote_ident(table_name)
    );
    let result = first_statement(api.sql(&sql).await?);
    let value = result
        .rows
        .first()
        .and_then(|row| row.first())
        .ok_or_else(|| "COUNT(*) returned no rows".to_string())?;
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .ok_or_else(|| format!("COUNT(*) returned an unexpected value: {value}"))
}

/// Refuses to mutate unless `predicate` identifies exactly one row. Without a
/// primary key the predicate is built from column values, which can match
/// duplicates or nothing at all if the row changed since it was loaded.
async fn ensure_single_match(api: &Api<'_>, table: &TableSummary, predicate: &str) -> Result<(), String> {
    let matches = count_rows(api, &table.name, &format!(" WHERE {predicate}")).await?;
    if matches == 1 {
        Ok(())
    } else {
        log::warn!("Aborted mutation on '{}': predicate matched {matches} rows", table.name);
        Err(format!(
            "Expected exactly 1 matching row but found {matches}. Nothing was changed; refresh and try again."
        ))
    }
}

#[tauri::command]
pub fn list_connections(state: tauri::State<'_, AppState>) -> Result<Vec<ConnectionProfile>, String> {
    state
        .profiles
        .lock()
        .map(|profiles| profiles.clone())
        .map_err(|_| "Connection state lock was poisoned".to_string())
}

#[tauri::command]
pub fn save_connection(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    input: SaveConnectionInput,
) -> Result<ConnectionProfile, String> {
    let name = input.name.trim();
    let database = input.database.trim();
    let base_url = profiles::normalize_url(&input.base_url);

    if name.is_empty() || database.is_empty() || base_url.is_empty() {
        return Err("Name, host URL, and database are required".into());
    }

    let token = if input.use_cli_token {
        Some(cli_config::token()?)
    } else {
        input
            .token
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
    };

    let mut profiles = state
        .profiles
        .lock()
        .map_err(|_| "Connection state lock was poisoned".to_string())?;
    let now = now_ms();
    let id = input
        .id
        .unwrap_or_else(|| profiles::new_connection_id(&profiles, now));
    let existing = profiles.iter().find(|profile| profile.id == id).cloned();

    if let Some(token) = token.as_deref() {
        profiles::set_token(&id, token)?;
    }

    let profile = ConnectionProfile {
        id: id.clone(),
        name: name.to_string(),
        base_url,
        database: database.to_string(),
        identity: None,
        has_token: token.is_some() || existing.as_ref().is_some_and(|profile| profile.has_token),
        read_only: input
            .read_only
            .or(existing.as_ref().map(|profile| profile.read_only))
            .unwrap_or(false),
        created_at: existing.as_ref().map_or(now, |profile| profile.created_at),
        updated_at: now,
    };

    match profiles.iter_mut().find(|profile| profile.id == id) {
        Some(slot) => *slot = profile.clone(),
        None => profiles.push(profile.clone()),
    }

    profiles::persist_profiles(&app, &profiles)?;
    log::info!(
        "Saved connection '{}' ({} / {}, read_only={})",
        profile.name,
        profile.base_url,
        profile.database,
        profile.read_only
    );
    Ok(profile)
}

#[tauri::command]
pub fn delete_connection(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<(), String> {
    let mut profiles = state
        .profiles
        .lock()
        .map_err(|_| "Connection state lock was poisoned".to_string())?;
    profiles.retain(|profile| profile.id != connection_id);
    profiles::persist_profiles(&app, &profiles)?;
    profiles::delete_token(&connection_id);
    log::info!("Deleted connection {connection_id}");
    Ok(())
}

#[tauri::command]
pub fn get_cli_config() -> Option<cli_config::CliConfigSummary> {
    cli_config::summary()
}

#[tauri::command]
pub async fn test_connection(
    state: tauri::State<'_, AppState>,
    input: TestConnectionInput,
) -> Result<TestConnectionResult, String> {
    let mut profile = match input.id.as_deref() {
        Some(id) => state.profile(id)?,
        None => ConnectionProfile {
            id: "test".into(),
            name: "Test".into(),
            base_url: String::new(),
            database: String::new(),
            identity: None,
            has_token: false,
            read_only: true,
            created_at: now_ms(),
            updated_at: now_ms(),
        },
    };
    if let Some(base_url) = input.base_url.as_deref() {
        profile.base_url = profiles::normalize_url(base_url);
    }
    if let Some(database) = input.database {
        profile.database = database.trim().to_string();
    }

    let override_token = if input.use_cli_token {
        Some(cli_config::token()?)
    } else {
        input.token.filter(|token| !token.trim().is_empty())
    };
    let api = match override_token {
        Some(token) => Api::with_token(&state.http, &profile, Some(token)),
        None if input.id.is_some() => Api::new(&state.http, &profile),
        None => Api::with_token(&state.http, &profile, None),
    };

    api.ping().await?;

    let (database_info, headers) = api
        .get_json("Database lookup", &api.database_path(""))
        .await?;

    Ok(TestConnectionResult {
        ok: true,
        identity: headers
            .get("spacetime-identity")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        database_identity: database_info
            .get("database_identity")
            .and_then(|value| value.get("__identity__").or(Some(value)))
            .and_then(Value::as_str)
            .map(str::to_string),
        message: "Connection succeeded".into(),
    })
}

#[tauri::command]
pub async fn get_schema(
    state: tauri::State<'_, AppState>,
    connection_id: String,
) -> Result<SchemaSummary, String> {
    let profile = state.profile(&connection_id)?;
    Api::new(&state.http, &profile).schema().await
}

#[tauri::command]
pub async fn query_table(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    table_name: String,
    page: usize,
    page_size: usize,
    where_clause: Option<String>,
) -> Result<TablePage, String> {
    let profile = state.profile(&connection_id)?;
    let api = Api::new(&state.http, &profile);
    let page_size = page_size.clamp(1, 500);
    let where_sql = sql::normalize_where_clause(where_clause.as_deref())?;
    let fetch_limit = ((page + 1) * page_size).min(MAX_BROWSE_ROWS);
    let select = format!(
        "SELECT * FROM {}{where_sql} LIMIT {};",
        sql::quote_ident(&table_name),
        fetch_limit + 1
    );

    let (rows, total) = tokio::join!(api.sql(&select), count_rows(&api, &table_name, &where_sql));
    let result = first_statement(rows?);
    let total = total
        .map_err(|error| log::warn!("Row count for '{table_name}' unavailable: {error}"))
        .ok();

    let start = (page * page_size).min(result.rows.len());
    let end = (start + page_size).min(fetch_limit).min(result.rows.len());
    let reached_limit = fetch_limit == MAX_BROWSE_ROWS && result.rows.len() > MAX_BROWSE_ROWS;

    Ok(TablePage {
        rows: rows_to_objects(&result.columns, &result.rows[start..end]),
        columns: result.columns,
        total,
        has_more: result.rows.len() > fetch_limit && !reached_limit,
        page,
        page_size,
        scan_limit: reached_limit.then_some(MAX_BROWSE_ROWS),
    })
}

#[tauri::command]
pub async fn execute_sql(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    sql: String,
) -> Result<SqlResult, String> {
    let profile = state.profile(&connection_id)?;
    if !sql::is_read_only_sql(&sql) {
        profile.ensure_writable()?;
    }
    Ok(SqlResult {
        statements: Api::new(&state.http, &profile).sql(&sql).await?,
    })
}

#[tauri::command]
pub async fn run_function(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    function_name: String,
    args: Value,
) -> Result<Value, String> {
    let profile = state.profile(&connection_id)?;
    profile.ensure_writable()?;

    if !args.is_array() {
        return Err("Function arguments must be sent as a JSON array".into());
    }

    Api::new(&state.http, &profile)
        .call_reducer(&function_name, &args)
        .await
}

#[tauri::command]
pub async fn create_row(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    table_name: String,
    row: Value,
) -> Result<(), String> {
    let profile = state.profile(&connection_id)?;
    profile.ensure_writable()?;
    let api = Api::new(&state.http, &profile);
    let table = table_summary(&api, &table_name).await?;

    api.sql(&sql::insert_statement(&table, &row)?).await?;
    Ok(())
}

/// `changes` holds only the columns to set; everything else is left untouched.
#[tauri::command]
pub async fn update_row(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    table_name: String,
    original_row: Value,
    changes: Value,
) -> Result<(), String> {
    let profile = state.profile(&connection_id)?;
    profile.ensure_writable()?;
    let api = Api::new(&state.http, &profile);
    let table = table_summary(&api, &table_name).await?;

    let assignments = sql::update_assignments(&table, &original_row, &changes)?;
    if assignments.is_empty() {
        return Ok(());
    }
    let predicate = sql::mutation_predicate(&table, &original_row)?;
    ensure_single_match(&api, &table, &predicate).await?;

    api.sql(&format!(
        "UPDATE {} SET {} WHERE {predicate};",
        sql::quote_ident(&table.name),
        assignments.join(", ")
    ))
    .await?;
    Ok(())
}

#[tauri::command]
pub async fn delete_row(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    table_name: String,
    row: Value,
) -> Result<(), String> {
    let profile = state.profile(&connection_id)?;
    profile.ensure_writable()?;
    let api = Api::new(&state.http, &profile);
    let table = table_summary(&api, &table_name).await?;

    let predicate = sql::mutation_predicate(&table, &row)?;
    ensure_single_match(&api, &table, &predicate).await?;

    api.sql(&format!(
        "DELETE FROM {} WHERE {predicate};",
        sql::quote_ident(&table.name)
    ))
    .await?;
    Ok(())
}

/// Starts streaming logs to `on_event` and returns an id for [`stop_log_stream`].
#[tauri::command]
pub fn start_log_stream(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    num_lines: Option<u32>,
    follow: bool,
    on_event: Channel<logs::LogEvent>,
) -> Result<u32, String> {
    let profile = state.profile(&connection_id)?;
    let num_lines = num_lines.unwrap_or(200).clamp(1, 10_000);
    let stream_id = state.next_stream_id.fetch_add(1, Ordering::Relaxed);
    let handle = tauri::async_runtime::spawn(logs::stream(
        state.http.clone(),
        profile,
        num_lines,
        follow,
        on_event,
    ));

    state
        .log_streams
        .lock()
        .map_err(|_| "Log stream lock was poisoned".to_string())?
        .insert(stream_id, handle);
    Ok(stream_id)
}

#[tauri::command]
pub fn stop_log_stream(state: tauri::State<'_, AppState>, stream_id: u32) -> Result<(), String> {
    if let Some(handle) = state
        .log_streams
        .lock()
        .map_err(|_| "Log stream lock was poisoned".to_string())?
        .remove(&stream_id)
    {
        handle.abort();
    }
    Ok(())
}

/// Saves rows the webview already holds, such as a SQL console result.
#[tauri::command]
pub async fn export_rows(
    app: AppHandle,
    default_name: String,
    format: ExportFormat,
    columns: Vec<ColumnSummary>,
    rows: Vec<Vec<Value>>,
) -> Result<Option<String>, String> {
    let contents = export::render(format, &columns, &rows);
    export::save_with_dialog(&app, &default_name, format, contents).await
}

/// Exports every row of a table matching the filter, up to [`MAX_EXPORT_ROWS`],
/// without routing the data through the webview.
#[tauri::command]
pub async fn export_table(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    connection_id: String,
    table_name: String,
    where_clause: Option<String>,
    format: ExportFormat,
) -> Result<Option<String>, String> {
    let profile = state.profile(&connection_id)?;
    let api = Api::new(&state.http, &profile);
    let where_sql = sql::normalize_where_clause(where_clause.as_deref())?;
    let result = first_statement(
        api.sql(&format!(
            "SELECT * FROM {}{where_sql} LIMIT {MAX_EXPORT_ROWS};",
            sql::quote_ident(&table_name)
        ))
        .await?,
    );

    let contents = export::render(format, &result.columns, &result.rows);
    export::save_with_dialog(&app, &table_name, format, contents).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema::ColumnKind;
    use serde_json::json;

    #[test]
    fn rows_become_objects_keyed_by_column_name() {
        let columns = vec![
            ColumnSummary { name: "id".into(), r#type: "U64".into(), kind: ColumnKind::Integer },
            ColumnSummary { name: "name".into(), r#type: "String".into(), kind: ColumnKind::String },
        ];
        let rows = vec![vec![json!(1), json!("a")], vec![json!(2)]];
        assert_eq!(
            rows_to_objects(&columns, &rows),
            vec![json!({ "id": 1, "name": "a" }), json!({ "id": 2, "name": null })]
        );
    }
}
