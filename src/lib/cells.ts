import type { ColumnKind, ColumnSummary } from "./spacetime";

// Kinds the backend can turn into a SQL literal. Everything else (timestamps,
// products, enums, arrays) is shown read-only in the row editors.
const EDITABLE_KINDS: ColumnKind[] = [
  "integer",
  "float",
  "bool",
  "string",
  "identity",
  "connectionId",
];

export function isEditable(column: ColumnSummary) {
  return EDITABLE_KINDS.includes(column.kind);
}

// SpacetimeDB wraps special types as `{"__identity__": x}` or `[x]` depending
// on the API, so pull out the single inner value either way.
function unwrapSpecial(value: unknown): unknown {
  if (Array.isArray(value) && value.length === 1) return value[0];
  if (value && typeof value === "object") {
    const values = Object.values(value as Record<string, unknown>);
    if (values.length === 1) return values[0];
  }
  return value;
}

function formatTimestamp(value: unknown) {
  const micros = Number(unwrapSpecial(value));
  if (!Number.isFinite(micros)) return JSON.stringify(value);
  const date = new Date(micros / 1000);
  return Number.isNaN(date.getTime()) ? String(micros) : date.toLocaleString();
}

// Text shown in a table cell.
export function formatCell(value: unknown, kind: ColumnKind = "other") {
  if (value === null || value === undefined) return "";
  if (kind === "timestamp") return formatTimestamp(value);
  if (kind === "identity" || kind === "connectionId") {
    return String(unwrapSpecial(value));
  }
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

// Text placed in an edit field. Identical to the display text except that
// timestamps keep their raw value, since they are not editable anyway.
export function cellToInput(value: unknown, column: ColumnSummary) {
  return column.kind === "timestamp"
    ? JSON.stringify(value ?? "")
    : formatCell(value, column.kind);
}

// Turns an edit field back into the value sent to the backend. Numbers stay as
// text so u64/i128 values keep every digit; the backend formats literals by the
// column's type.
export function parseInput(text: string, column: ColumnSummary): unknown {
  const trimmed = text.trim();
  switch (column.kind) {
    case "bool":
      return trimmed === "true";
    case "integer":
    case "float":
    case "identity":
    case "connectionId":
      return trimmed;
    case "string":
      return text;
    default:
      try {
        return JSON.parse(trimmed);
      } catch {
        return text;
      }
  }
}

// A value for a reducer argument. Reducer calls take SATS-JSON, so numbers
// must be real JSON numbers there.
export function parseArg(text: string, column: ColumnSummary): unknown {
  const trimmed = text.trim();
  switch (column.kind) {
    case "integer":
    case "float":
      return trimmed === "" ? 0 : Number(trimmed);
    case "bool":
      return trimmed === "true";
    case "string":
    case "identity":
    case "connectionId":
      return text;
    default:
      try {
        return JSON.parse(trimmed);
      } catch {
        return text;
      }
  }
}

export type BadgeColor = "info" | "success" | "warning" | "neutral";

export function kindBadgeColor(column: ColumnSummary): BadgeColor {
  switch (column.kind) {
    case "string":
      return "info";
    case "integer":
    case "float":
      return "success";
    case "bool":
      return "warning";
    default:
      return "neutral";
  }
}
