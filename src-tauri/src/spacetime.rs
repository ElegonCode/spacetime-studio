use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

const KEYRING_SERVICE: &str = "spacetime-studio";
const DEFAULT_SCHEMA_VERSION: u16 = 9;

#[derive(Default)]
pub struct AppState {
    pub profiles: Mutex<Vec<ConnectionProfile>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub database: String,
    pub identity: Option<String>,
    pub has_token: bool,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConnectionInput {
    pub id: Option<String>,
    pub name: String,
    pub base_url: String,
    pub database: String,
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionInput {
    pub id: Option<String>,
    pub base_url: Option<String>,
    pub database: Option<String>,
    pub token: Option<String>,
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
pub struct SchemaSummary {
    pub raw: Value,
    pub tables: Vec<TableSummary>,
    pub reducers: Vec<FunctionSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSummary {
    pub name: String,
    pub table_type: String,
    pub access: String,
    pub columns: Vec<ColumnSummary>,
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSummary {
    pub name: String,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSummary {
    pub name: String,
    pub lifecycle: Option<String>,
    pub params: Vec<ColumnSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TablePage {
    pub columns: Vec<ColumnSummary>,
    pub rows: Vec<Value>,
    pub total: Option<u64>,
    pub has_more: bool,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SqlResult {
    pub results: Value,
}

fn now_ms() -> u64 {
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
    let Ok(path) = profiles_path(app) else {
        return Vec::new();
    };

    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };

    serde_json::from_str(&contents).unwrap_or_default()
}

fn persist_profiles(app: &AppHandle, profiles: &[ConnectionProfile]) -> Result<(), String> {
    let path = profiles_path(app)?;
    let json = serde_json::to_string_pretty(profiles)
        .map_err(|error| format!("Could not serialize connections: {error}"))?;
    fs::write(path, json).map_err(|error| format!("Could not store connections: {error}"))
}

fn normalize_url(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

fn token_key(connection_id: &str) -> String {
    format!("connection:{connection_id}")
}

fn set_token(connection_id: &str, token: &str) -> Result<(), String> {
    keyring::Entry::new(KEYRING_SERVICE, &token_key(connection_id))
        .map_err(|error| format!("Could not open OS credential store: {error}"))?
        .set_password(token)
        .map_err(|error| format!("Could not store token securely: {error}"))
}

fn get_token(connection_id: &str) -> Option<String> {
    keyring::Entry::new(KEYRING_SERVICE, &token_key(connection_id))
        .ok()
        .and_then(|entry| entry.get_password().ok())
}

fn delete_token(connection_id: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &token_key(connection_id)) {
        let _ = entry.delete_credential();
    }
}

fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn normalize_where_clause(where_clause: Option<&str>) -> Result<String, String> {
    let Some(where_clause) = where_clause
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(String::new());
    };

    if where_clause.contains(';') {
        return Err("WHERE query cannot contain semicolons.".into());
    }

    let lowercase_where = where_clause.to_ascii_lowercase();
    if lowercase_where == "where" {
        Ok(String::new())
    } else if lowercase_where.starts_with("where ")
        || lowercase_where.starts_with("where\t")
        || lowercase_where.starts_with("where\r")
        || lowercase_where.starts_with("where\n")
    {
        let condition = where_clause[5..].trim_start();
        Ok(format!(" WHERE {condition}"))
    } else {
        Ok(format!(" WHERE {where_clause}"))
    }
}

fn sql_literal(value: &Value) -> Result<String, String> {
    match value {
        Value::Null => {
            Err("NULL values are not supported by the current Spacetime SQL literal grammar".into())
        }
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) => Ok(format!("'{}'", value.replace('\'', "''"))),
        _ => Err("Only scalar values can be used in generic SQL mutations".into()),
    }
}

fn profile_by_id(state: &AppState, connection_id: &str) -> Result<ConnectionProfile, String> {
    state
        .profiles
        .lock()
        .map_err(|_| "Connection state lock was poisoned".to_string())?
        .iter()
        .find(|profile| profile.id == connection_id)
        .cloned()
        .ok_or_else(|| "Connection profile not found".to_string())
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent("spacetime-studio/0.1")
        .build()
        .map_err(|error| format!("Could not create HTTP client: {error}"))
}

async fn get_json(
    profile: &ConnectionProfile,
    path: &str,
    token_override: Option<&str>,
) -> Result<(Value, reqwest::header::HeaderMap), String> {
    let url = format!("{}{}", profile.base_url, path);
    let mut request = client()?.get(url);
    let stored_token = if token_override.is_none() {
        get_token(&profile.id)
    } else {
        None
    };
    if let Some(token) = token_override.or(stored_token.as_deref()) {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("HTTP request failed: {error}"))?;
    let headers = response.headers().clone();
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read HTTP response: {error}"))?;

    if !status.is_success() {
        return Err(format!("SpacetimeDB returned {status}: {body}"));
    }

    let value = if body.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&body)
            .map_err(|error| format!("Invalid JSON response: {error}; body: {body}"))?
    };

    Ok((value, headers))
}

async fn post_sql(profile: &ConnectionProfile, sql: &str) -> Result<Value, String> {
    let url = format!("{}/v1/database/{}/sql", profile.base_url, profile.database);
    let mut request = client()?
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "text/plain")
        .body(sql.to_string());

    if let Some(token) = get_token(&profile.id) {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("SQL request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read SQL response: {error}"))?;

    if !status.is_success() {
        return Err(format!("SpacetimeDB SQL returned {status}: {body}"));
    }

    serde_json::from_str(&body)
        .map_err(|error| format!("Invalid SQL JSON response: {error}; body: {body}"))
}

async fn post_reducer_call(
    profile: &ConnectionProfile,
    function_name: &str,
    args: Value,
) -> Result<Value, String> {
    let url = format!(
        "{}/v1/database/{}/call/{}",
        profile.base_url, profile.database, function_name
    );
    let mut request = client()?.post(url).json(&args);

    if let Some(token) = get_token(&profile.id) {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("Function request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read function response: {error}"))?;

    if !status.is_success() {
        return Err(format!("SpacetimeDB function returned {status}: {body}"));
    }

    if body.trim().is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_str(&body)
            .map_err(|error| format!("Invalid function JSON response: {error}; body: {body}"))
    }
}

fn variant_name(value: &Value) -> String {
    value
        .as_object()
        .and_then(|object| object.keys().next().cloned())
        .unwrap_or_else(|| "unknown".to_string())
}

fn type_name(value: &Value) -> String {
    match value {
        Value::Object(object) => object
            .iter()
            .next()
            .map(|(key, inner)| {
                if key == "Ref" {
                    format!("type_ref({})", inner.as_u64().unwrap_or_default())
                } else {
                    key.clone()
                }
            })
            .unwrap_or_else(|| "unknown".to_string()),
        _ => "unknown".to_string(),
    }
}

fn product_elements(raw: &Value, product_type_ref: usize) -> Vec<ColumnSummary> {
    raw.pointer(&format!(
        "/typespace/types/{product_type_ref}/Product/elements"
    ))
    .and_then(Value::as_array)
    .map(|elements| {
        elements
            .iter()
            .enumerate()
            .map(|(index, element)| {
                let name = element
                    .pointer("/name/some")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("field_{index}"));
                let ty = element
                    .get("algebraic_type")
                    .map(type_name)
                    .unwrap_or_else(|| "unknown".to_string());
                ColumnSummary { name, r#type: ty }
            })
            .collect()
    })
    .unwrap_or_default()
}

fn primary_key_names(table: &Value, columns: &[ColumnSummary]) -> Vec<String> {
    table
        .get("primary_key")
        .and_then(Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter_map(|key| {
                    key.as_str().map(str::to_string).or_else(|| {
                        key.as_u64()
                            .and_then(|index| columns.get(index as usize))
                            .map(|column| column.name.clone())
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn summarize_schema(raw: Value) -> SchemaSummary {
    let tables = raw
        .get("tables")
        .and_then(Value::as_array)
        .map(|tables| {
            tables
                .iter()
                .map(|table| {
                    let product_type_ref = table
                        .get("product_type_ref")
                        .and_then(Value::as_u64)
                        .unwrap_or_default() as usize;
                    let columns = product_elements(&raw, product_type_ref);

                    TableSummary {
                        name: table
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                        table_type: table
                            .get("table_type")
                            .map(variant_name)
                            .unwrap_or_else(|| "unknown".to_string()),
                        access: table
                            .get("table_access")
                            .map(variant_name)
                            .unwrap_or_else(|| "unknown".to_string()),
                        primary_key: primary_key_names(table, &columns),
                        columns,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let reducers = raw
        .get("reducers")
        .and_then(Value::as_array)
        .map(|reducers| {
            reducers
                .iter()
                .map(|reducer| {
                    let params = reducer
                        .pointer("/params/elements")
                        .and_then(Value::as_array)
                        .map(|elements| {
                            elements
                                .iter()
                                .enumerate()
                                .map(|(index, element)| ColumnSummary {
                                    name: element
                                        .pointer("/name/some")
                                        .and_then(Value::as_str)
                                        .map(str::to_string)
                                        .unwrap_or_else(|| format!("arg_{index}")),
                                    r#type: element
                                        .get("algebraic_type")
                                        .map(type_name)
                                        .unwrap_or_else(|| "unknown".to_string()),
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    FunctionSummary {
                        name: reducer
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                        lifecycle: reducer.get("lifecycle").map(variant_name),
                        params,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    SchemaSummary {
        raw,
        tables,
        reducers,
    }
}

fn rows_to_objects(columns: &[ColumnSummary], rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            let Some(values) = row.as_array() else {
                return row.clone();
            };

            let object = columns
                .iter()
                .enumerate()
                .map(|(index, column)| {
                    (
                        column.name.clone(),
                        values.get(index).cloned().unwrap_or(Value::Null),
                    )
                })
                .collect();

            Value::Object(object)
        })
        .collect()
}

async fn table_summary(
    profile: &ConnectionProfile,
    table_name: &str,
) -> Result<TableSummary, String> {
    let schema = get_schema_for_profile(profile).await?;
    schema
        .tables
        .into_iter()
        .find(|table| table.name == table_name)
        .ok_or_else(|| format!("Table '{table_name}' was not found in the schema"))
}

async fn get_schema_for_profile(profile: &ConnectionProfile) -> Result<SchemaSummary, String> {
    let (raw, _) = get_json(
        profile,
        &format!(
            "/v1/database/{}/schema?version={DEFAULT_SCHEMA_VERSION}",
            profile.database
        ),
        None,
    )
    .await?;
    Ok(summarize_schema(raw))
}

fn mutation_predicate(table: &TableSummary, row: &Value) -> Result<String, String> {
    let Some(object) = row.as_object() else {
        return Err("Row identity must be a JSON object".into());
    };

    let keys = if table.primary_key.is_empty() {
        table
            .columns
            .iter()
            .map(|column| column.name.clone())
            .collect::<Vec<_>>()
    } else {
        table.primary_key.clone()
    };

    if keys.is_empty() {
        return Err("Could not derive a row identity predicate".into());
    }

    keys.iter()
        .map(|key| {
            let value = object
                .get(key)
                .ok_or_else(|| format!("Row identity is missing '{key}'"))?;
            Ok(format!("{} = {}", quote_ident(key), sql_literal(value)?))
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|parts| parts.join(" AND "))
}

#[tauri::command]
pub fn list_connections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ConnectionProfile>, String> {
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
    let base_url = normalize_url(&input.base_url);

    if name.is_empty() || database.is_empty() || base_url.is_empty() {
        return Err("Name, host URL, and database are required".into());
    }

    let mut profiles = state
        .profiles
        .lock()
        .map_err(|_| "Connection state lock was poisoned".to_string())?;
    let now = now_ms();
    let id = input.id.unwrap_or_else(|| format!("conn-{now}"));
    let token = input
        .token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty());

    if let Some(token) = token.as_deref() {
        set_token(&id, token)?;
    }

    let has_token = token.is_some()
        || profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.has_token)
            .unwrap_or(false);

    let profile = ConnectionProfile {
        id: id.clone(),
        name: name.to_string(),
        base_url,
        database: database.to_string(),
        identity: None,
        has_token,
        created_at: profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.created_at)
            .unwrap_or(now),
        updated_at: now,
    };

    if let Some(existing) = profiles.iter_mut().find(|existing| existing.id == id) {
        *existing = profile.clone();
    } else {
        profiles.push(profile.clone());
    }

    persist_profiles(&app, &profiles)?;
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
    persist_profiles(&app, &profiles)?;
    delete_token(&connection_id);
    Ok(())
}

#[tauri::command]
pub async fn test_connection(
    state: tauri::State<'_, AppState>,
    input: TestConnectionInput,
) -> Result<TestConnectionResult, String> {
    let mut profile = if let Some(id) = input.id.as_deref() {
        profile_by_id(&state, id)?
    } else {
        ConnectionProfile {
            id: "test".into(),
            name: "Test".into(),
            base_url: normalize_url(input.base_url.as_deref().unwrap_or_default()),
            database: input.database.clone().unwrap_or_default(),
            identity: None,
            has_token: input
                .token
                .as_ref()
                .is_some_and(|token| !token.trim().is_empty()),
            created_at: now_ms(),
            updated_at: now_ms(),
        }
    };

    if let Some(base_url) = input.base_url.as_deref() {
        profile.base_url = normalize_url(base_url);
    }
    if let Some(database) = input.database {
        profile.database = database;
    }

    let ping_url = format!("{}/v1/ping", profile.base_url);
    let ping = client()?
        .get(ping_url)
        .send()
        .await
        .map_err(|error| format!("Could not reach SpacetimeDB host: {error}"))?;

    if !ping.status().is_success() {
        return Ok(TestConnectionResult {
            ok: false,
            identity: None,
            database_identity: None,
            message: format!("Host responded with {}", ping.status()),
        });
    }

    let (database_info, headers) = get_json(
        &profile,
        &format!("/v1/database/{}", profile.database),
        input.token.as_deref(),
    )
    .await?;

    Ok(TestConnectionResult {
        ok: true,
        identity: headers
            .get("spacetime-identity")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        database_identity: database_info
            .get("database_identity")
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
    let profile = profile_by_id(&state, &connection_id)?;
    get_schema_for_profile(&profile).await
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
    let profile = profile_by_id(&state, &connection_id)?;
    let table = table_summary(&profile, &table_name).await?;
    let safe_page_size = page_size.clamp(1, 100);
    let fetch_limit = ((page + 1) * safe_page_size).clamp(1, 1000);
    let probe_limit = (fetch_limit + 1).min(1001);
    let table_ident = quote_ident(&table.name);
    let where_sql = normalize_where_clause(where_clause.as_deref())?;
    let sql = format!("SELECT * FROM {table_ident}{where_sql} LIMIT {probe_limit};");
    let results = post_sql(&profile, &sql).await?;
    let statements = results.as_array().cloned().unwrap_or_default();
    let all_rows = statements
        .first()
        .and_then(|statement| statement.get("rows"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has_more = all_rows.len() > fetch_limit;
    let start = page.saturating_mul(safe_page_size);
    let end = (start + safe_page_size)
        .min(fetch_limit)
        .min(all_rows.len());
    let rows = if start < all_rows.len() {
        rows_to_objects(&table.columns, &all_rows[start..end])
    } else {
        Vec::new()
    };

    Ok(TablePage {
        columns: table.columns,
        rows,
        total: None,
        has_more,
        page,
        page_size: safe_page_size,
    })
}

#[tauri::command]
pub async fn execute_sql(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    sql: String,
) -> Result<SqlResult, String> {
    let profile = profile_by_id(&state, &connection_id)?;
    Ok(SqlResult {
        results: post_sql(&profile, &sql).await?,
    })
}

#[tauri::command]
pub async fn run_function(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    function_name: String,
    args: Value,
) -> Result<SqlResult, String> {
    let profile = profile_by_id(&state, &connection_id)?;

    if !args.is_array() {
        return Err("Function arguments must be sent as a JSON array".into());
    }

    Ok(SqlResult {
        results: post_reducer_call(&profile, &function_name, args).await?,
    })
}

#[tauri::command]
pub async fn create_row(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    table_name: String,
    row: Value,
) -> Result<SqlResult, String> {
    let profile = profile_by_id(&state, &connection_id)?;
    let table = table_summary(&profile, &table_name).await?;
    let object = row
        .as_object()
        .ok_or_else(|| "New row must be a JSON object".to_string())?;

    let mut columns = Vec::new();
    let mut values = Vec::new();

    for column in &table.columns {
        if let Some(value) = object.get(&column.name) {
            columns.push(quote_ident(&column.name));
            values.push(sql_literal(value)?);
        }
    }

    if columns.is_empty() {
        return Err("At least one column value is required".into());
    }

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({});",
        quote_ident(&table.name),
        columns.join(", "),
        values.join(", ")
    );

    Ok(SqlResult {
        results: post_sql(&profile, &sql).await?,
    })
}

#[tauri::command]
pub async fn update_row(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    table_name: String,
    original_row: Value,
    updated_row: Value,
) -> Result<SqlResult, String> {
    let profile = profile_by_id(&state, &connection_id)?;
    let table = table_summary(&profile, &table_name).await?;
    let object = updated_row
        .as_object()
        .ok_or_else(|| "Updated row must be a JSON object".to_string())?;

    let assignments = table
        .columns
        .iter()
        .filter_map(|column| {
            object.get(&column.name).map(|value| {
                Ok(format!(
                    "{} = {}",
                    quote_ident(&column.name),
                    sql_literal(value)?
                ))
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    if assignments.is_empty() {
        return Err("At least one column value is required".into());
    }

    let sql = format!(
        "UPDATE {} SET {} WHERE {};",
        quote_ident(&table.name),
        assignments.join(", "),
        mutation_predicate(&table, &original_row)?
    );

    Ok(SqlResult {
        results: post_sql(&profile, &sql).await?,
    })
}

#[tauri::command]
pub async fn delete_row(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    table_name: String,
    row: Value,
) -> Result<SqlResult, String> {
    let profile = profile_by_id(&state, &connection_id)?;
    let table = table_summary(&profile, &table_name).await?;
    let sql = format!(
        "DELETE FROM {} WHERE {};",
        quote_ident(&table.name),
        mutation_predicate(&table, &row)?
    );

    Ok(SqlResult {
        results: post_sql(&profile, &sql).await?,
    })
}

#[tauri::command]
pub async fn get_logs(
    state: tauri::State<'_, AppState>,
    connection_id: String,
    num_lines: Option<u16>,
) -> Result<String, String> {
    let profile = profile_by_id(&state, &connection_id)?;
    let lines = num_lines.unwrap_or(200);
    let url = format!(
        "{}/v1/database/{}/logs?num_lines={lines}",
        profile.base_url, profile.database
    );
    let mut request = client()?.get(url);

    if let Some(token) = get_token(&profile.id) {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("Log request failed: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read log response: {error}"))?;

    if status != StatusCode::OK {
        return Err(format!("SpacetimeDB logs returned {status}: {body}"));
    }

    Ok(body)
}
