<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  createRow,
  executeSql,
  getSchema,
  getSelectedConnectionId,
  parseCell,
  queryTable,
  removeRow,
  stringifyCell,
  updateRow,
  type SchemaSummary,
  type TablePage,
  type TableSummary,
} from "../lib/spacetime";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";

const connectionId = ref<string | null>(getSelectedConnectionId());
const schema = ref<SchemaSummary | null>(null);
const selectedTableName = ref("");
const tablePage = ref<TablePage | null>(null);
const page = ref(0);
const pageSize = ref(25);
const loadingSchema = ref(false);
const loadingRows = ref(false);
const savingRow = ref(false);
const error = ref("");
const status = ref("");
const rowJson = ref("");
const originalRow = ref<Record<string, unknown> | null>(null);
const rowEditorOpen = ref(false);
const sql = ref("");
const sqlResult = ref("");
const runningSql = ref(false);

const tables = computed(() => schema.value?.tables ?? []);
const selectedTable = computed<TableSummary | null>(
  () =>
    tables.value.find((table) => table.name === selectedTableName.value) ??
    null,
);

function defaultRow(table: TableSummary) {
  return Object.fromEntries(table.columns.map((column) => [column.name, ""]));
}

function formatJson(value: unknown) {
  return JSON.stringify(value, null, 2);
}

async function loadSchema() {
  connectionId.value = getSelectedConnectionId();
  if (!connectionId.value) {
    error.value = "Select or create a connection first.";
    return;
  }

  loadingSchema.value = true;
  error.value = "";

  try {
    schema.value = await getSchema(connectionId.value);
    selectedTableName.value =
      selectedTableName.value || schema.value.tables[0]?.name || "";
    if (selectedTableName.value) await loadRows();
  } catch (err) {
    error.value = String(err);
  } finally {
    loadingSchema.value = false;
  }
}

async function loadRows() {
  if (!connectionId.value || !selectedTableName.value) return;

  loadingRows.value = true;
  error.value = "";
  status.value = "";

  try {
    tablePage.value = await queryTable(
      connectionId.value,
      selectedTableName.value,
      page.value,
      pageSize.value,
    );
  } catch (err) {
    error.value = String(err);
  } finally {
    loadingRows.value = false;
  }
}

function newRow() {
  if (!selectedTable.value) return;
  originalRow.value = null;
  rowJson.value = formatJson(defaultRow(selectedTable.value));
  rowEditorOpen.value = true;
}

function editRow(row: Record<string, unknown>) {
  originalRow.value = { ...row };
  rowJson.value = formatJson(row);
  rowEditorOpen.value = true;
}

async function saveRow() {
  if (!connectionId.value || !selectedTableName.value) return;

  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(rowJson.value);
  } catch (err) {
    error.value = `Invalid row JSON: ${String(err)}`;
    return;
  }

  savingRow.value = true;
  error.value = "";
  status.value = "";

  try {
    if (originalRow.value) {
      await updateRow(
        connectionId.value,
        selectedTableName.value,
        originalRow.value,
        parsed,
      );
      status.value = "Row updated.";
    } else {
      await createRow(connectionId.value, selectedTableName.value, parsed);
      status.value = "Row inserted.";
    }
    rowJson.value = "";
    originalRow.value = null;
    rowEditorOpen.value = false;
    await loadRows();
  } catch (err) {
    error.value = String(err);
  } finally {
    savingRow.value = false;
  }
}

async function deleteRow(row: Record<string, unknown>) {
  if (!connectionId.value || !selectedTableName.value) return;

  savingRow.value = true;
  error.value = "";
  status.value = "";

  try {
    await removeRow(connectionId.value, selectedTableName.value, row);
    status.value = "Row deleted.";
    await loadRows();
  } catch (err) {
    error.value = String(err);
  } finally {
    savingRow.value = false;
  }
}

async function runSql() {
  if (!connectionId.value) {
    error.value = "Select or create a connection first.";
    return;
  }

  runningSql.value = true;
  error.value = "";
  status.value = "";

  try {
    const result = await executeSql(connectionId.value, sql.value);
    sqlResult.value = formatJson(result.results);
    status.value = "SQL executed.";
    if (selectedTableName.value) await loadRows();
  } catch (err) {
    error.value = String(err);
  } finally {
    runningSql.value = false;
  }
}

function updateCell(
  row: Record<string, unknown>,
  columnName: string,
  value: string,
) {
  row[columnName] = parseCell(value);
}

watch(selectedTableName, () => {
  page.value = 0;
  rowJson.value = "";
  originalRow.value = null;
  rowEditorOpen.value = false;
  loadRows();
});

watch(pageSize, () => {
  page.value = 0;
  loadRows();
});

onMounted(() => {
  loadSchema();
  setPageRefreshHandler(loadSchema);
});

onUnmounted(() => {
  clearPageRefreshHandler(loadSchema);
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-4">
    <UAlert
      v-if="error"
      color="error"
      variant="subtle"
      :description="error"
      class="shrink-0"
    />
    <UAlert
      v-if="status"
      color="success"
      variant="subtle"
      :description="status"
      class="shrink-0"
    />

    <div
      class="grid min-h-0 flex-1 gap-4 xl:grid-rows-[minmax(0,1fr)_minmax(180px,28%)]"
    >
      <div class="grid min-h-0 gap-4 xl:grid-cols-[320px_1fr]">
        <aside
          class="flex min-h-0 flex-col rounded-lg border border-default bg-default/30 p-4"
        >
          <div class="mb-3 shrink-0 flex items-center justify-between gap-3">
            <h2 class="text-base font-semibold text-highlighted">Schema</h2>
            <UBadge color="neutral" variant="subtle"
              >{{ tables.length }} tables</UBadge
            >
          </div>

          <div class="min-h-0 flex-1 space-y-2 overflow-auto pr-1">
            <button
              v-for="table in tables"
              :key="table.name"
              class="w-full rounded-md border px-3 py-2 text-left transition"
              :class="
                table.name === selectedTableName
                  ? 'border-primary bg-primary/10'
                  : 'border-default bg-default/20 hover:bg-default/50'
              "
              @click="selectedTableName = table.name"
            >
              <div class="flex items-center justify-between gap-3">
                <span class="truncate text-sm font-medium text-highlighted">{{
                  table.name
                }}</span>
                <UBadge size="sm" color="neutral" variant="subtle">{{
                  table.access
                }}</UBadge>
              </div>
              <p class="mt-1 truncate text-xs text-muted">
                {{ table.columns.length }} columns
                <span v-if="table.primaryKey.length">
                  - PK {{ table.primaryKey.join(", ") }}</span
                >
              </p>
            </button>
          </div>
        </aside>

        <section
          class="min-h-0 min-w-0 flex flex-col rounded-lg border border-default bg-default/30"
        >
          <div
            class="shrink-0 flex flex-wrap items-center justify-between gap-3 border-b border-default p-4"
          >
            <div class="min-w-0">
              <h2 class="truncate text-base font-semibold text-highlighted">
                {{ selectedTableName || "No table selected" }}
              </h2>
              <p class="mt-1 text-xs text-muted">
                {{
                  tablePage?.total == null
                    ? "Row count unknown"
                    : `${tablePage.total} rows`
                }}
                - page {{ page + 1 }}
              </p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <USelect
                v-model="pageSize"
                :items="[10, 25, 50, 100]"
                class="w-24"
                aria-label="Page size"
              />
              <UButton
                icon="i-lucide-chevron-left"
                color="neutral"
                variant="soft"
                :disabled="page === 0"
                aria-label="Previous page"
                @click="
                  page--;
                  loadRows();
                "
              />
              <UButton
                icon="i-lucide-chevron-right"
                color="neutral"
                variant="soft"
                :disabled="!tablePage?.hasMore"
                aria-label="Next page"
                @click="
                  page++;
                  loadRows();
                "
              />
              <UButton icon="i-lucide-plus" @click="newRow">New Row</UButton>
            </div>
          </div>

          <div class="min-h-0 flex-1 overflow-auto">
            <table class="min-w-full text-sm">
              <thead class="bg-default/60">
                <tr>
                  <th
                    v-for="column in tablePage?.columns ?? []"
                    :key="column.name"
                    class="whitespace-nowrap border-b border-default px-3 py-2 text-left text-xs font-medium text-muted"
                  >
                    {{ column.name }}
                    <span class="font-normal">({{ column.type }})</span>
                  </th>
                  <th
                    class="sticky right-0 z-20 w-28 border-b border-default bg-default/95 px-3 py-2 text-right text-xs font-medium text-muted shadow-[-12px_0_18px_-18px_rgba(0,0,0,0.9)]"
                  >
                    Actions
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="loadingRows">
                  <td
                    class="px-3 py-6 text-muted"
                    :colspan="(tablePage?.columns.length ?? 0) + 1"
                  >
                    Loading rows...
                  </td>
                </tr>
                <tr
                  v-for="(row, rowIndex) in tablePage?.rows ?? []"
                  :key="rowIndex"
                  class="border-b border-default/60"
                >
                  <td
                    v-for="column in tablePage?.columns ?? []"
                    :key="column.name"
                    class="min-w-40 px-3 py-2 align-top"
                  >
                    <input
                      class="w-full rounded border border-transparent bg-transparent px-2 py-1 text-highlighted outline-none focus:border-primary focus:bg-default"
                      :value="stringifyCell(row[column.name])"
                      @change="
                        updateCell(
                          row,
                          column.name,
                          ($event.target as HTMLInputElement).value,
                        )
                      "
                    />
                  </td>
                  <td
                    class="sticky right-0 z-10 whitespace-nowrap bg-default/95 px-3 py-2 text-right align-top shadow-[-12px_0_18px_-18px_rgba(0,0,0,0.9)]"
                  >
                    <UButton
                      icon="i-lucide-save"
                      size="xs"
                      variant="ghost"
                      aria-label="Save row"
                      @click="
                        editRow(row);
                        saveRow();
                      "
                    />
                    <UButton
                      icon="i-lucide-file-pen-line"
                      size="xs"
                      color="neutral"
                      variant="ghost"
                      aria-label="Edit JSON"
                      @click="editRow(row)"
                    />
                    <UButton
                      icon="i-lucide-trash-2"
                      size="xs"
                      color="error"
                      variant="ghost"
                      aria-label="Delete row"
                      @click="deleteRow(row)"
                    />
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>
      </div>

      <section
        class="flex min-h-0 flex-col rounded-lg border border-default bg-default/30 p-4"
      >
        <div class="mb-3 shrink-0 flex items-center justify-between gap-3">
          <h2 class="text-base font-semibold text-highlighted">Raw SQL</h2>
          <UButton
            icon="i-lucide-play"
            :loading="runningSql"
            :disabled="!sql.trim()"
            @click="runSql"
          >
            Run
          </UButton>
        </div>
        <textarea
          v-model="sql"
          placeholder="SELECT * FROM my_table LIMIT 10;"
          class="flex-1 min-h-9 resize-none rounded-md border border-default bg-default px-3 py-2 font-mono text-xs text-highlighted outline-none focus:border-primary"
        />
        <pre
          class="mt-3 h-14 shrink-0 overflow-auto rounded-md border border-default bg-black p-3 text-xs text-neutral-100"
          >{{ sqlResult || "SQL results will appear here." }}</pre
        >
      </section>
    </div>

    <USlideover
      v-model:open="rowEditorOpen"
      :title="originalRow ? 'Edit Row JSON' : 'Create Row JSON'"
      :description="selectedTableName"
      side="right"
      :ui="{
        content: 'sm:max-w-2xl',
        body: 'min-h-0 flex-1',
        footer: 'shrink-0',
      }"
    >
      <template #body>
        <textarea
          v-model="rowJson"
          placeholder="{ }"
          class="h-full min-h-[420px] w-full resize-none overflow-auto rounded-md border border-default bg-default px-3 py-2 font-mono text-xs text-highlighted outline-none focus:border-primary"
        />
      </template>

      <template #footer="{ close }">
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="close"
            >Cancel</UButton
          >
          <UButton
            icon="i-lucide-save"
            :loading="savingRow"
            :disabled="!rowJson"
            @click="saveRow"
          >
            Save Row
          </UButton>
        </div>
      </template>
    </USlideover>
  </div>
</template>
