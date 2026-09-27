import { Channel, invoke } from "@tauri-apps/api/core";

export type ConnectionProfile = {
  id: string;
  name: string;
  baseUrl: string;
  database: string;
  identity?: string | null;
  hasToken: boolean;
  readOnly: boolean;
  createdAt: number;
  updatedAt: number;
};

// How a column's values are written in SQL and shown in the UI, derived from
// its algebraic type by the backend.
export type ColumnKind =
  | "integer"
  | "float"
  | "bool"
  | "string"
  | "identity"
  | "connectionId"
  | "timestamp"
  | "other";

export type ColumnSummary = {
  name: string;
  type: string;
  kind: ColumnKind;
};

export type TableSummary = {
  name: string;
  tableType: string;
  access: string;
  columns: ColumnSummary[];
  primaryKey: string[];
  scheduledReducer?: string | null;
};

export type FunctionSummary = {
  name: string;
  lifecycle?: string | null;
  params: ColumnSummary[];
  scheduledBy?: string | null;
};

export type SchemaSummary = {
  raw: unknown;
  tables: TableSummary[];
  reducers: FunctionSummary[];
};

export type Row = Record<string, unknown>;

export type TablePage = {
  columns: ColumnSummary[];
  rows: Row[];
  total?: number | null;
  hasMore: boolean;
  page: number;
  pageSize: number;
  scanLimit?: number | null;
};

export type TestConnectionResult = {
  ok: boolean;
  identity?: string | null;
  databaseIdentity?: string | null;
  message: string;
};

export type SqlStatementResult = {
  columns: ColumnSummary[];
  rows: unknown[][];
  durationMicros?: number | null;
};

export type SqlResult = {
  statements: SqlStatementResult[];
};

export type CliConfigSummary = {
  path: string;
  hasToken: boolean;
  servers: { nickname: string; url: string; isDefault: boolean }[];
};

export type LogEvent =
  | { kind: "lines"; lines: string[] }
  | { kind: "error"; message: string }
  | { kind: "end" };

export type ExportFormat = "csv" | "json";

const SELECTED_CONNECTION_KEY = "spacetime-studio:selected-connection";

export function getSelectedConnectionId() {
  return localStorage.getItem(SELECTED_CONNECTION_KEY);
}

export function setSelectedConnectionId(id: string) {
  localStorage.setItem(SELECTED_CONNECTION_KEY, id);
}

export function clearSelectedConnectionId() {
  localStorage.removeItem(SELECTED_CONNECTION_KEY);
}

export function listConnections() {
  return invoke<ConnectionProfile[]>("list_connections");
}

export function saveConnection(input: {
  id?: string;
  name: string;
  baseUrl: string;
  database: string;
  token?: string;
  useCliToken?: boolean;
  readOnly?: boolean;
}) {
  return invoke<ConnectionProfile>("save_connection", { input });
}

export function deleteConnection(connectionId: string) {
  return invoke<void>("delete_connection", { connectionId });
}

export function testConnection(input: {
  id?: string;
  baseUrl?: string;
  database?: string;
  token?: string;
  useCliToken?: boolean;
}) {
  return invoke<TestConnectionResult>("test_connection", { input });
}

export function getCliConfig() {
  return invoke<CliConfigSummary | null>("get_cli_config");
}

export function getSchema(connectionId: string) {
  return invoke<SchemaSummary>("get_schema", { connectionId });
}

export function queryTable(
  connectionId: string,
  tableName: string,
  page: number,
  pageSize: number,
  where?: string,
) {
  return invoke<TablePage>("query_table", {
    connectionId,
    tableName,
    page,
    pageSize,
    whereClause: where,
  });
}

export function executeSql(connectionId: string, sql: string) {
  return invoke<SqlResult>("execute_sql", { connectionId, sql });
}

export function runFunction(
  connectionId: string,
  functionName: string,
  args: unknown[],
) {
  return invoke<unknown>("run_function", {
    connectionId,
    functionName,
    args,
  });
}

export function createRow(connectionId: string, tableName: string, row: Row) {
  return invoke<void>("create_row", { connectionId, tableName, row });
}

// `changes` holds only the columns to set. The backend identifies the row from
// `originalRow` and refuses to write unless exactly one row matches.
export function updateRow(
  connectionId: string,
  tableName: string,
  originalRow: Row,
  changes: Row,
) {
  return invoke<void>("update_row", {
    connectionId,
    tableName,
    originalRow,
    changes,
  });
}

export function removeRow(connectionId: string, tableName: string, row: Row) {
  return invoke<void>("delete_row", { connectionId, tableName, row });
}

// Streams log lines as they are written (or just the last `numLines` when not
// following). Returns a function that stops the stream.
export async function streamLogs(
  connectionId: string,
  options: { numLines: number; follow: boolean },
  onEvent: (event: LogEvent) => void,
) {
  const channel = new Channel<LogEvent>(onEvent);
  const streamId = await invoke<number>("start_log_stream", {
    connectionId,
    numLines: options.numLines,
    follow: options.follow,
    onEvent: channel,
  });
  return () => invoke<void>("stop_log_stream", { streamId });
}

// Both exports open a native save dialog and resolve to the written path, or
// null when the user cancels.
export function exportRows(
  defaultName: string,
  format: ExportFormat,
  columns: ColumnSummary[],
  rows: unknown[][],
) {
  return invoke<string | null>("export_rows", {
    defaultName,
    format,
    columns,
    rows,
  });
}

export function exportTable(
  connectionId: string,
  tableName: string,
  format: ExportFormat,
  where?: string,
) {
  return invoke<string | null>("export_table", {
    connectionId,
    tableName,
    format,
    whereClause: where,
  });
}
