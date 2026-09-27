//! Turns the raw v9 module definition (`/v1/database/:name/schema?version=9`)
//! into the summaries the frontend renders, including readable SATS type names.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How a column's values are written in SQL and rendered in the UI. Derived from
/// the column's algebraic type rather than its display name, so named aliases and
/// special SpacetimeDB types are classified correctly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColumnKind {
    Integer,
    Float,
    Bool,
    String,
    Identity,
    ConnectionId,
    Timestamp,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSummary {
    pub name: String,
    pub r#type: String,
    pub kind: ColumnKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSummary {
    pub name: String,
    pub table_type: String,
    pub access: String,
    pub columns: Vec<ColumnSummary>,
    pub primary_key: Vec<String>,
    /// The reducer this schedule table fires, when it is one.
    pub scheduled_reducer: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSummary {
    pub name: String,
    /// `Init`, `OnConnect` or `OnDisconnect` for lifecycle reducers, which the
    /// HTTP API refuses to call directly.
    pub lifecycle: Option<String>,
    pub params: Vec<ColumnSummary>,
    /// The schedule table that fires this reducer, when one does.
    pub scheduled_by: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSummary {
    pub raw: Value,
    pub tables: Vec<TableSummary>,
    pub reducers: Vec<FunctionSummary>,
}

const INTEGER_TYPES: &[&str] = &[
    "U8", "U16", "U32", "U64", "U128", "U256", "I8", "I16", "I32", "I64", "I128", "I256",
];

/// Resolves type references against the module's typespace and names them
/// using the module's exported type names.
pub struct TypeContext<'a> {
    raw: Option<&'a Value>,
}

impl<'a> TypeContext<'a> {
    pub fn new(raw: &'a Value) -> Self {
        Self { raw: Some(raw) }
    }

    /// For algebraic types that are already fully inline, such as the column
    /// schema returned alongside SQL results.
    pub fn inline() -> Self {
        Self { raw: None }
    }

    fn resolve_ref(&self, index: u64) -> Option<&'a Value> {
        self.raw?.pointer(&format!("/typespace/types/{index}"))
    }

    fn ref_name(&self, index: u64) -> Option<String> {
        self.raw?
            .get("types")?
            .as_array()?
            .iter()
            .find(|def| def.get("ty").and_then(Value::as_u64) == Some(index))
            .and_then(|def| def.pointer("/name/name"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    pub fn describe(&self, ty: &Value) -> (String, ColumnKind) {
        self.describe_depth(ty, 0)
    }

    fn describe_depth(&self, ty: &Value, depth: usize) -> (String, ColumnKind) {
        if depth > 16 {
            return ("…".into(), ColumnKind::Other);
        }

        let Some((tag, inner)) = ty.as_object().and_then(|object| object.iter().next()) else {
            return ("unknown".into(), ColumnKind::Other);
        };

        match tag.as_str() {
            tag if INTEGER_TYPES.contains(&tag) => (tag.to_string(), ColumnKind::Integer),
            "F32" | "F64" => (tag.clone(), ColumnKind::Float),
            "Bool" => (tag.clone(), ColumnKind::Bool),
            "String" => (tag.clone(), ColumnKind::String),
            "Ref" => {
                let index = inner.as_u64().unwrap_or_default();
                let resolved = self
                    .resolve_ref(index)
                    .map(|resolved| self.describe_depth(resolved, depth + 1));
                match (self.ref_name(index), resolved) {
                    (Some(name), Some((_, kind))) => (name, kind),
                    (Some(name), None) => (name, ColumnKind::Other),
                    (None, Some(described)) => described,
                    (None, None) => (format!("type_ref({index})"), ColumnKind::Other),
                }
            }
            "Array" => {
                let (element, _) = self.describe_depth(inner, depth + 1);
                if element == "U8" {
                    ("Bytes".into(), ColumnKind::Other)
                } else {
                    (format!("Array<{element}>"), ColumnKind::Other)
                }
            }
            "Product" => describe_product(inner),
            "Sum" => self.describe_sum(inner, depth),
            other => (other.to_string(), ColumnKind::Other),
        }
    }

    fn describe_sum(&self, sum: &Value, depth: usize) -> (String, ColumnKind) {
        let variants = sum
            .get("variants")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let names: Vec<String> = variants
            .iter()
            .enumerate()
            .map(|(index, variant)| element_name(variant, index, "variant"))
            .collect();
        let payload = |index: usize| {
            variants
                .get(index)
                .and_then(|variant| variant.get("algebraic_type"))
                .map(|ty| self.describe_depth(ty, depth + 1).0)
                .unwrap_or_else(|| "unknown".into())
        };

        let name = match names.iter().map(String::as_str).collect::<Vec<_>>()[..] {
            ["some", "none"] => format!("Option<{}>", payload(0)),
            ["ok", "err"] => format!("Result<{}, {}>", payload(0), payload(1)),
            ["Interval", "Time"] => "ScheduleAt".into(),
            _ => "Enum".into(),
        };
        (name, ColumnKind::Other)
    }
}

/// SpacetimeDB encodes its special types as single-field products whose field
/// name marks the type.
fn describe_product(product: &Value) -> (String, ColumnKind) {
    let elements = product
        .get("elements")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();

    if let [only] = elements {
        match only.pointer("/name/some").and_then(Value::as_str) {
            Some("__identity__") => return ("Identity".into(), ColumnKind::Identity),
            Some("__connection_id__") => return ("ConnectionId".into(), ColumnKind::ConnectionId),
            Some("__timestamp_micros_since_unix_epoch__") => {
                return ("Timestamp".into(), ColumnKind::Timestamp)
            }
            Some("__time_duration_micros__") => return ("TimeDuration".into(), ColumnKind::Other),
            Some("__uuid__") => return ("Uuid".into(), ColumnKind::Other),
            _ => {}
        }
    }

    ("Product".into(), ColumnKind::Other)
}

/// Element names are SATS options: `{"some": "name"}` or `{"none": []}`.
fn element_name(element: &Value, index: usize, fallback_prefix: &str) -> String {
    let name = element.get("name");
    name.and_then(|name| name.get("some"))
        .or(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{fallback_prefix}_{index}"))
}

/// Unwraps a SATS option, returning `None` for `{"none": []}`.
fn option_inner(value: &Value) -> Option<&Value> {
    let object = value.as_object()?;
    if let Some(inner) = object.get("some") {
        return Some(inner);
    }
    if object.contains_key("none") {
        return None;
    }
    Some(value)
}

fn variant_name(value: &Value) -> String {
    value
        .as_object()
        .and_then(|object| object.keys().next().cloned())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn product_columns(context: &TypeContext, elements: &[Value], prefix: &str) -> Vec<ColumnSummary> {
    elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let (r#type, kind) = element
                .get("algebraic_type")
                .map(|ty| context.describe(ty))
                .unwrap_or_else(|| ("unknown".into(), ColumnKind::Other));
            ColumnSummary {
                name: element_name(element, index, prefix),
                r#type,
                kind,
            }
        })
        .collect()
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

pub fn summarize_schema(raw: Value) -> SchemaSummary {
    let context = TypeContext::new(&raw);

    let tables: Vec<TableSummary> = raw
        .get("tables")
        .and_then(Value::as_array)
        .map(|tables| {
            tables
                .iter()
                .map(|table| {
                    let product_type_ref = table
                        .get("product_type_ref")
                        .and_then(Value::as_u64)
                        .unwrap_or_default();
                    let elements = raw
                        .pointer(&format!("/typespace/types/{product_type_ref}/Product/elements"))
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    let columns = product_columns(&context, elements, "field");

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
                        scheduled_reducer: table
                            .get("schedule")
                            .and_then(option_inner)
                            .and_then(|schedule| schedule.get("reducer_name"))
                            .and_then(Value::as_str)
                            .map(str::to_string),
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
                    let name = reducer
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    let elements = reducer
                        .pointer("/params/elements")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default();

                    FunctionSummary {
                        lifecycle: reducer
                            .get("lifecycle")
                            .and_then(option_inner)
                            .map(variant_name),
                        params: product_columns(&context, elements, "arg"),
                        scheduled_by: tables
                            .iter()
                            .find(|table| table.scheduled_reducer.as_deref() == Some(&name))
                            .map(|table| table.name.clone()),
                        name,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    log::info!(
        "Schema summarized: {} table(s), {} reducer(s)",
        tables.len(),
        raw.get("reducers").and_then(Value::as_array).map_or(0, Vec::len)
    );

    SchemaSummary {
        raw,
        tables,
        reducers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn element(name: &str, ty: Value) -> Value {
        json!({ "name": { "some": name }, "algebraic_type": ty })
    }

    fn sample_schema() -> Value {
        json!({
            "typespace": { "types": [
                { "Product": { "elements": [
                    element("id", json!({ "U64": [] })),
                    element("owner", json!({ "Product": { "elements": [element("__identity__", json!({ "U256": [] }))] } })),
                    element("created", json!({ "Product": { "elements": [element("__timestamp_micros_since_unix_epoch__", json!({ "I64": [] }))] } })),
                    element("nickname", json!({ "Sum": { "variants": [
                        element("some", json!({ "String": [] })),
                        element("none", json!({ "Product": { "elements": [] } })),
                    ] } })),
                    element("position", json!({ "Ref": 1 })),
                    element("tags", json!({ "Array": { "String": [] } })),
                ] } },
                { "Product": { "elements": [element("x", json!({ "F32": [] }))] } },
                { "Product": { "elements": [
                    element("scheduled_id", json!({ "U64": [] })),
                    element("scheduled_at", json!({ "Sum": { "variants": [
                        element("Interval", json!({ "U64": [] })),
                        element("Time", json!({ "U64": [] })),
                    ] } })),
                ] } }
            ] },
            "types": [ { "name": { "scope": [], "name": "Vec2" }, "ty": 1, "custom_ordering": true } ],
            "tables": [
                {
                    "name": "player",
                    "product_type_ref": 0,
                    "primary_key": [0],
                    "table_type": { "User": [] },
                    "table_access": { "Public": [] },
                    "schedule": { "none": [] }
                },
                {
                    "name": "tick_timer",
                    "product_type_ref": 2,
                    "primary_key": [0],
                    "table_type": { "User": [] },
                    "table_access": { "Private": [] },
                    "schedule": { "some": { "name": { "none": [] }, "reducer_name": "tick", "scheduled_at_column": 1 } }
                }
            ],
            "reducers": [
                { "name": "init", "params": { "elements": [] }, "lifecycle": { "some": { "Init": [] } } },
                { "name": "tick", "params": { "elements": [element("timer", json!({ "Ref": 2 }))] }, "lifecycle": { "none": [] } }
            ]
        })
    }

    #[test]
    fn columns_get_readable_type_names_and_kinds() {
        let summary = summarize_schema(sample_schema());
        let columns = &summary.tables[0].columns;
        let described: Vec<(&str, &str, ColumnKind)> = columns
            .iter()
            .map(|column| (column.name.as_str(), column.r#type.as_str(), column.kind))
            .collect();

        assert_eq!(
            described,
            vec![
                ("id", "U64", ColumnKind::Integer),
                ("owner", "Identity", ColumnKind::Identity),
                ("created", "Timestamp", ColumnKind::Timestamp),
                ("nickname", "Option<String>", ColumnKind::Other),
                ("position", "Vec2", ColumnKind::Other),
                ("tags", "Array<String>", ColumnKind::Other),
            ]
        );
        assert_eq!(summary.tables[0].primary_key, vec!["id"]);
    }

    #[test]
    fn lifecycle_is_unwrapped_from_its_option_encoding() {
        let summary = summarize_schema(sample_schema());
        assert_eq!(summary.reducers[0].lifecycle.as_deref(), Some("Init"));
        assert_eq!(summary.reducers[1].lifecycle, None);
    }

    #[test]
    fn schedule_tables_are_linked_to_their_reducer() {
        let summary = summarize_schema(sample_schema());
        assert_eq!(summary.tables[0].scheduled_reducer, None);
        assert_eq!(summary.tables[1].scheduled_reducer.as_deref(), Some("tick"));
        assert_eq!(summary.reducers[1].scheduled_by.as_deref(), Some("tick_timer"));
        assert_eq!(summary.tables[1].columns[1].r#type, "ScheduleAt");
    }
}
