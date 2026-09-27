//! Builds the SQL the row editor sends. Values are formatted from the column's
//! type rather than from the JSON shape the webview happened to produce, so a
//! string column holding "123" stays a string and a large u64 keeps its digits.

use serde_json::Value;

use super::schema::{ColumnKind, ColumnSummary, TableSummary};

/// JavaScript numbers lose precision past 2^53, so larger integers are sent to
/// the webview as strings and parsed back by column type.
const MAX_SAFE_JS_INTEGER: u64 = (1 << 53) - 1;

pub fn quote_ident(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub fn normalize_where_clause(where_clause: Option<&str>) -> Result<String, String> {
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
    } else if lowercase_where.starts_with("where")
        && where_clause[5..].starts_with(char::is_whitespace)
    {
        Ok(format!(" WHERE {}", where_clause[5..].trim_start()))
    } else {
        Ok(format!(" WHERE {where_clause}"))
    }
}

fn is_integer_text(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
}

/// Identity and ConnectionId values show up as a bare hex string, a single-field
/// product object, or a single-element array depending on the API that produced
/// them. All of them become a `0x…` hex literal.
fn hex_literal(value: &Value) -> Option<String> {
    let raw = match value {
        Value::String(text) => text.as_str(),
        Value::Object(object) => object
            .get("__identity__")
            .or_else(|| object.get("__connection_id__"))
            .and_then(Value::as_str)?,
        Value::Array(items) if items.len() == 1 => items[0].as_str()?,
        _ => return None,
    };
    let hex = raw
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    (!hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("0x{hex}"))
}

pub fn sql_literal(value: &Value, column: &ColumnSummary) -> Result<String, String> {
    let name = &column.name;
    match (column.kind, value) {
        (_, Value::Null) => Err(format!("Column '{name}' can't be set to NULL")),
        (ColumnKind::Bool, Value::Bool(flag)) => Ok(flag.to_string()),
        (ColumnKind::Bool, Value::String(text)) if matches!(text.trim(), "true" | "false") => {
            Ok(text.trim().to_string())
        }
        (ColumnKind::Integer, Value::Number(number)) if number.is_i64() || number.is_u64() => {
            Ok(number.to_string())
        }
        (ColumnKind::Integer, Value::String(text)) if is_integer_text(text.trim()) => {
            Ok(text.trim().to_string())
        }
        (ColumnKind::Float, Value::Number(number)) => Ok(number.to_string()),
        (ColumnKind::Float, Value::String(text))
            if text.trim().parse::<f64>().is_ok_and(f64::is_finite) =>
        {
            Ok(text.trim().to_string())
        }
        (ColumnKind::String, Value::String(text)) => Ok(quote_string(text)),
        (ColumnKind::String, Value::Number(_) | Value::Bool(_)) => {
            Ok(quote_string(&value.to_string()))
        }
        (ColumnKind::Identity | ColumnKind::ConnectionId, _) => hex_literal(value)
            .ok_or_else(|| format!("Column '{name}' expects a hex {} value", column.r#type)),
        (ColumnKind::Integer | ColumnKind::Float | ColumnKind::Bool, _) => Err(format!(
            "Column '{name}' expects a {} value",
            column.r#type
        )),
        _ => Err(format!(
            "Column '{name}' has type {}, which generic SQL edits can't write",
            column.r#type
        )),
    }
}

fn column<'a>(table: &'a TableSummary, name: &str) -> Result<&'a ColumnSummary, String> {
    table
        .columns
        .iter()
        .find(|column| column.name == name)
        .ok_or_else(|| format!("Table '{}' has no column '{name}'", table.name))
}

/// A WHERE predicate that identifies `row`. Uses the primary key when the table
/// has one, and otherwise every column whose value can be written as a literal.
/// Callers must still confirm it matches exactly one row before mutating.
pub fn mutation_predicate(table: &TableSummary, row: &Value) -> Result<String, String> {
    let Some(object) = row.as_object() else {
        return Err("Row identity must be a JSON object".into());
    };

    if !table.primary_key.is_empty() {
        return table
            .primary_key
            .iter()
            .map(|key| {
                let value = object
                    .get(key)
                    .ok_or_else(|| format!("Row identity is missing '{key}'"))?;
                Ok(format!(
                    "{} = {}",
                    quote_ident(key),
                    sql_literal(value, column(table, key)?)?
                ))
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|parts| parts.join(" AND "));
    }

    let parts: Vec<String> = table
        .columns
        .iter()
        .filter_map(|column| {
            let literal = sql_literal(object.get(&column.name)?, column).ok()?;
            Some(format!("{} = {literal}", quote_ident(&column.name)))
        })
        .collect();

    if parts.is_empty() {
        Err("Could not derive a row identity predicate from SQL-compatible values".into())
    } else {
        Ok(parts.join(" AND "))
    }
}

/// `SET` assignments for the columns in `changes` whose value differs from
/// `original`. Only touching changed columns means a row with, say, a
/// Timestamp column can still have its other fields edited.
pub fn update_assignments(
    table: &TableSummary,
    original: &Value,
    changes: &Value,
) -> Result<Vec<String>, String> {
    let changes = changes
        .as_object()
        .ok_or_else(|| "Updated row must be a JSON object".to_string())?;

    changes
        .iter()
        .filter(|(name, value)| original.get(name.as_str()) != Some(*value))
        .map(|(name, value)| {
            Ok(format!(
                "{} = {}",
                quote_ident(name),
                sql_literal(value, column(table, name)?)?
            ))
        })
        .collect()
}

pub fn insert_statement(table: &TableSummary, row: &Value) -> Result<String, String> {
    let object = row
        .as_object()
        .ok_or_else(|| "New row must be a JSON object".to_string())?;

    let mut columns = Vec::new();
    let mut values = Vec::new();
    for column in &table.columns {
        if let Some(value) = object.get(&column.name) {
            columns.push(quote_ident(&column.name));
            values.push(sql_literal(value, column)?);
        }
    }

    if columns.is_empty() {
        return Err("At least one column value is required".into());
    }

    Ok(format!(
        "INSERT INTO {} ({}) VALUES ({});",
        quote_ident(&table.name),
        columns.join(", "),
        values.join(", ")
    ))
}

/// Whether every statement in `sql` only reads. Deliberately conservative: a
/// semicolon inside a string literal splits the text, and the resulting
/// fragment is then treated as a write.
pub fn is_read_only_sql(sql: &str) -> bool {
    let statements: Vec<String> = sql
        .split(';')
        .map(|statement| {
            statement
                .lines()
                .skip_while(|line| {
                    let line = line.trim();
                    line.is_empty() || line.starts_with("--")
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|statement| !statement.trim().is_empty())
        .collect();

    !statements.is_empty()
        && statements.iter().all(|statement| {
            let keyword = statement
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            matches!(keyword.as_str(), "select" | "show" | "explain")
        })
}

/// Integers past JavaScript's safe range become strings so the webview keeps
/// every digit. [`sql_literal`] turns them back into bare numbers by column type.
pub fn js_safe_value(value: Value) -> Value {
    match &value {
        Value::Number(number)
            if number.as_u64().is_some_and(|n| n > MAX_SAFE_JS_INTEGER)
                || number
                    .as_i64()
                    .is_some_and(|n| n < -(MAX_SAFE_JS_INTEGER as i64)) =>
        {
            Value::String(number.to_string())
        }
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn col(name: &str, r#type: &str, kind: ColumnKind) -> ColumnSummary {
        ColumnSummary {
            name: name.into(),
            r#type: r#type.into(),
            kind,
        }
    }

    fn players(primary_key: Vec<&str>) -> TableSummary {
        TableSummary {
            name: "players".into(),
            table_type: "User".into(),
            access: "Public".into(),
            columns: vec![
                col("id", "U64", ColumnKind::Integer),
                col("name", "String", ColumnKind::String),
                col("stats", "Stats", ColumnKind::Other),
            ],
            primary_key: primary_key.into_iter().map(str::to_string).collect(),
            scheduled_reducer: None,
        }
    }

    #[test]
    fn literals_follow_the_column_type_not_the_json_shape() {
        let string = col("name", "String", ColumnKind::String);
        let integer = col("id", "U64", ColumnKind::Integer);

        assert_eq!(sql_literal(&json!("123"), &string).unwrap(), "'123'");
        assert_eq!(sql_literal(&json!("O'Brien"), &string).unwrap(), "'O''Brien'");
        assert_eq!(
            sql_literal(&json!("18446744073709551615"), &integer).unwrap(),
            "18446744073709551615"
        );
        assert!(sql_literal(&json!("abc"), &integer).is_err());
    }

    #[test]
    fn identities_become_hex_literals() {
        let identity = col("owner", "Identity", ColumnKind::Identity);
        for value in [
            json!("0xC200ab"),
            json!({ "__identity__": "0xc200ab" }),
            json!(["c200ab"]),
        ] {
            assert_eq!(
                sql_literal(&value, &identity).unwrap().to_lowercase(),
                "0xc200ab"
            );
        }
        assert!(sql_literal(&json!("not hex"), &identity).is_err());
    }

    #[test]
    fn mutation_predicate_without_primary_key_uses_writable_columns() {
        let row = json!({ "id": 7, "name": "Ada", "stats": { "wins": 3 } });
        assert_eq!(
            mutation_predicate(&players(Vec::new()), &row).unwrap(),
            "\"id\" = 7 AND \"name\" = 'Ada'"
        );
    }

    #[test]
    fn mutation_predicate_with_primary_key_uses_only_the_key() {
        let row = json!({ "id": 7, "name": "Ada", "stats": { "wins": 3 } });
        assert_eq!(
            mutation_predicate(&players(vec!["id"]), &row).unwrap(),
            "\"id\" = 7"
        );
    }

    #[test]
    fn mutation_predicate_with_unwritable_primary_key_errors() {
        let row = json!({ "id": 7, "name": "Ada", "stats": { "wins": 3 } });
        assert!(mutation_predicate(&players(vec!["stats"]), &row)
            .unwrap_err()
            .contains("Stats"));
    }

    #[test]
    fn mutation_predicate_without_usable_columns_errors_clearly() {
        let row = json!({ "id": null, "name": null, "stats": { "wins": 3 } });
        assert_eq!(
            mutation_predicate(&players(Vec::new()), &row).unwrap_err(),
            "Could not derive a row identity predicate from SQL-compatible values"
        );
    }

    #[test]
    fn update_only_sets_changed_columns() {
        let original = json!({ "id": 7, "name": "Ada", "stats": { "wins": 3 } });
        let changes = json!({ "id": 7, "name": "Grace" });
        assert_eq!(
            update_assignments(&players(vec!["id"]), &original, &changes).unwrap(),
            vec!["\"name\" = 'Grace'"]
        );
    }

    #[test]
    fn read_only_detection_is_conservative() {
        assert!(is_read_only_sql("SELECT * FROM player"));
        assert!(is_read_only_sql("-- look\n  select 1; SHOW x;"));
        assert!(!is_read_only_sql("DELETE FROM player"));
        assert!(!is_read_only_sql("SELECT 1; DELETE FROM player"));
        assert!(!is_read_only_sql("SELECT '--'; DELETE FROM player"));
        assert!(!is_read_only_sql("   "));
    }

    #[test]
    fn where_clause_accepts_an_optional_where_keyword() {
        assert_eq!(normalize_where_clause(Some("id = 1")).unwrap(), " WHERE id = 1");
        assert_eq!(normalize_where_clause(Some("WHERE id = 1")).unwrap(), " WHERE id = 1");
        assert_eq!(
            normalize_where_clause(Some("whereabouts = 1")).unwrap(),
            " WHERE whereabouts = 1"
        );
        assert_eq!(normalize_where_clause(Some("where")).unwrap(), "");
        assert!(normalize_where_clause(Some("1=1; DROP")).is_err());
    }

    #[test]
    fn large_integers_are_sent_to_the_webview_as_strings() {
        assert_eq!(js_safe_value(json!(42)), json!(42));
        assert_eq!(
            js_safe_value(json!(18446744073709551615u64)),
            json!("18446744073709551615")
        );
    }
}
