import { invoke } from "@tauri-apps/api/core";

export type ConnectionProfile = {
  id: string;
  name: string;
  baseUrl: string;
  database: string;
  identity?: string | null;
  hasToken: boolean;
  createdAt: number;
  updatedAt: number;
};

export type ColumnSummary = {
  name: string;
  type: string;
};

export type TableSummary = {
  name: string;
  tableType: string;
  access: string;
  columns: ColumnSummary[];
  primaryKey: string[];
};

export type FunctionSummary = {
  name: string;
  lifecycle?: string | null;
  params: ColumnSummary[];
};

export type SchemaSummary = {
  raw: unknown;
  tables: TableSummary[];
  reducers: FunctionSummary[];
};

export type TablePage = {
  columns: ColumnSummary[];
  rows: Record<string, unknown>[];
  total?: number | null;
  hasMore: boolean;
  page: number;
  pageSize: number;
};

export type TestConnectionResult = {
  ok: boolean;
  identity?: string | null;
  databaseIdentity?: string | null;
  message: string;
};

export type SqlResult = {
  results: unknown;
};

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
}) {
  return invoke<TestConnectionResult>("test_connection", { input });
}

export function getSchema(connectionId: string) {
  return invoke<SchemaSummary>("get_schema", { connectionId });
}

export function queryTable(
  connectionId: string,
  tableName: string,
  page: number,
  pageSize: number,
) {
  return invoke<TablePage>("query_table", {
    connectionId,
    tableName,
    page,
    pageSize,
  });
}

export function executeSql(connectionId: string, sql: string) {
  return invoke<SqlResult>("execute_sql", { connectionId, sql });
}

export function createRow(
  connectionId: string,
  tableName: string,
  row: Record<string, unknown>,
) {
  return invoke<SqlResult>("create_row", { connectionId, tableName, row });
}

export function updateRow(
  connectionId: string,
  tableName: string,
  originalRow: Record<string, unknown>,
  updatedRow: Record<string, unknown>,
) {
  return invoke<SqlResult>("update_row", {
    connectionId,
    tableName,
    originalRow,
    updatedRow,
  });
}

export function removeRow(
  connectionId: string,
  tableName: string,
  row: Record<string, unknown>,
) {
  return invoke<SqlResult>("delete_row", { connectionId, tableName, row });
}

export function getLogs(connectionId: string, numLines = 200) {
  return invoke<string>("get_logs", { connectionId, numLines });
}

export function stringifyCell(value: unknown) {
  if (value === null || value === undefined) return "";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

export function parseCell(value: string) {
  const trimmed = value.trim();
  if (trimmed === "") return "";
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  if (/^-?\d+(\.\d+)?$/.test(trimmed)) return Number(trimmed);

  try {
    return JSON.parse(trimmed);
  } catch {
    return value;
  }
}
