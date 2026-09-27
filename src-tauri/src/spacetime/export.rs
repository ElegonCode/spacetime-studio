//! CSV/JSON export of result sets, written wherever the user picks in a
//! native save dialog.

use serde::Deserialize;
use serde_json::{Map, Value};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use super::schema::{ColumnKind, ColumnSummary};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
        }
    }
}

pub fn render(format: ExportFormat, columns: &[ColumnSummary], rows: &[Vec<Value>]) -> String {
    match format {
        ExportFormat::Csv => build_csv(columns, rows),
        ExportFormat::Json => build_json(columns, rows),
    }
}

fn escape_csv_field(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// The innermost scalar of a special-type wrapper such as `{"__identity__": x}` or `[x]`.
fn unwrap_special(value: &Value) -> &Value {
    match value {
        Value::Object(object) if object.len() == 1 => object.values().next().unwrap_or(value),
        Value::Array(items) if items.len() == 1 => &items[0],
        _ => value,
    }
}

fn cell_text(value: &Value, kind: ColumnKind) -> String {
    match kind {
        ColumnKind::Identity | ColumnKind::ConnectionId => match unwrap_special(value) {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        },
        ColumnKind::Timestamp => unwrap_special(value)
            .as_i64()
            .map(format_timestamp_micros)
            .unwrap_or_else(|| value.to_string()),
        _ => match value {
            Value::Null => String::new(),
            Value::String(text) => text.clone(),
            other => other.to_string(),
        },
    }
}

fn build_csv(columns: &[ColumnSummary], rows: &[Vec<Value>]) -> String {
    let mut out = columns
        .iter()
        .map(|column| escape_csv_field(&column.name))
        .collect::<Vec<_>>()
        .join(",");
    out.push('\n');

    for row in rows {
        let fields: Vec<String> = columns
            .iter()
            .enumerate()
            .map(|(index, column)| {
                escape_csv_field(&cell_text(row.get(index).unwrap_or(&Value::Null), column.kind))
            })
            .collect();
        out.push_str(&fields.join(","));
        out.push('\n');
    }

    out
}

fn build_json(columns: &[ColumnSummary], rows: &[Vec<Value>]) -> String {
    let objects: Vec<Value> = rows
        .iter()
        .map(|row| {
            let object: Map<String, Value> = columns
                .iter()
                .enumerate()
                .map(|(index, column)| {
                    (column.name.clone(), row.get(index).cloned().unwrap_or(Value::Null))
                })
                .collect();
            Value::Object(object)
        })
        .collect();

    serde_json::to_string_pretty(&objects).unwrap_or_default()
}

/// RFC 3339 in UTC, without pulling in a date library.
fn format_timestamp_micros(micros: i64) -> String {
    let seconds = micros.div_euclid(1_000_000);
    let micros = micros.rem_euclid(1_000_000);
    let days = seconds.div_euclid(86_400);
    let secs_of_day = seconds.rem_euclid(86_400);

    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{micros:06}Z",
        secs_of_day / 3_600,
        secs_of_day % 3_600 / 60,
        secs_of_day % 60
    )
}

/// Shows a save dialog and writes `contents` to the chosen file. Returns the
/// path written, or `None` when the user cancelled.
pub async fn save_with_dialog(
    app: &AppHandle,
    default_stem: &str,
    format: ExportFormat,
    contents: String,
) -> Result<Option<String>, String> {
    let extension = format.extension();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(format!("{default_stem}.{extension}"))
        .add_filter(extension.to_uppercase(), &[extension])
        .save_file(move |path| {
            let _ = sender.send(path);
        });

    let Some(path) = receiver
        .await
        .map_err(|_| "The save dialog closed unexpectedly".to_string())?
    else {
        return Ok(None);
    };
    let path = path
        .into_path()
        .map_err(|error| format!("Unsupported save location: {error}"))?;

    std::fs::write(&path, contents)
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    log::info!("Exported {} to {}", extension, path.display());
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn col(name: &str, kind: ColumnKind) -> ColumnSummary {
        ColumnSummary {
            name: name.into(),
            r#type: String::new(),
            kind,
        }
    }

    #[test]
    fn csv_escapes_and_formats_special_types() {
        let columns = vec![
            col("name", ColumnKind::String),
            col("owner", ColumnKind::Identity),
            col("at", ColumnKind::Timestamp),
        ];
        let rows = vec![vec![
            json!("a, \"b\""),
            json!({ "__identity__": "0xc2" }),
            json!([1_700_000_000_123_456i64]),
        ]];

        assert_eq!(
            build_csv(&columns, &rows),
            "name,owner,at\n\"a, \"\"b\"\"\",0xc2,2023-11-14T22:13:20.123456Z\n"
        );
    }

    #[test]
    fn timestamps_before_the_epoch_format_correctly() {
        assert_eq!(format_timestamp_micros(-1), "1969-12-31T23:59:59.999999Z");
        assert_eq!(format_timestamp_micros(0), "1970-01-01T00:00:00.000000Z");
    }
}
