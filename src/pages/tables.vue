<script setup lang="ts">
import type { TabsItem } from "@nuxt/ui";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  createRow,
  getSchema,
  getSelectedConnectionId,
  parseCell,
  queryTable,
  removeRow,
  stringifyCell,
  updateRow,
  type ColumnSummary,
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
type RowFormValue = string | boolean;

const rowForm = ref<Record<string, RowFormValue>>({});
const originalRow = ref<Record<string, unknown> | null>(null);
const rowEditorOpen = ref(false);
const whereSql = ref("");
const activeWhereSql = ref("");
const tableSearch = ref("");

const tables = computed(() => schema.value?.tables ?? []);
const filteredTables = computed(() => {
  const search = tableSearch.value.trim().toLowerCase();
  if (!search) return tables.value;

  return tables.value.filter((table) =>
    table.name.toLowerCase().includes(search),
  );
});
const tableItems = computed<TabsItem[]>(() =>
  filteredTables.value.map((table) => {
    const isPrivate = table.access.toLowerCase() === "private";

    return {
      label: table.name,
      value: table.name,
      icon: isPrivate ? "i-lucide-lock" : "i-lucide-table",
      ui: isPrivate ? { leadingIcon: "text-error" } : undefined,
    };
  }),
);
const selectedTable = computed<TableSummary | null>(
  () =>
    tables.value.find((table) => table.name === selectedTableName.value) ??
    null,
);

function defaultRow(table: TableSummary) {
  return Object.fromEntries(
    table.columns.map((column) => [
      column.name,
      isBooleanColumn(column) ? false : "",
    ]),
  );
}

function normalizeType(type: string) {
  return type.toLowerCase().replace(/[^a-z0-9]/g, "");
}

function isBooleanColumn(column: ColumnSummary) {
  return ["bool", "boolean"].includes(normalizeType(column.type));
}

function isNumberColumn(column: ColumnSummary) {
  return (
    /^(u|i)(8|16|32|64|128)$/.test(normalizeType(column.type)) ||
    /^(f)(32|64)$/.test(normalizeType(column.type)) ||
    ["usize", "isize"].includes(normalizeType(column.type))
  );
}

function rowToForm(row: Record<string, unknown>, table: TableSummary) {
  return Object.fromEntries(
    table.columns.map((column) => [
      column.name,
      isBooleanColumn(column)
        ? row[column.name] === true ||
          stringifyCell(row[column.name]) === "true"
        : stringifyCell(row[column.name]),
    ]),
  );
}

function formToRow(table: TableSummary) {
  return Object.fromEntries(
    table.columns.map((column) => [
      column.name,
      isBooleanColumn(column)
        ? rowForm.value[column.name] === true
        : parseCell(String(rowForm.value[column.name] ?? "")),
    ]),
  );
}

function getTextField(columnName: string) {
  return String(rowForm.value[columnName] ?? "");
}

function setTextField(columnName: string, value: string | number) {
  rowForm.value[columnName] = String(value);
}

function getBooleanField(columnName: string) {
  return rowForm.value[columnName] === true;
}

function setBooleanField(columnName: string, value: boolean | "indeterminate") {
  rowForm.value[columnName] = value === true;
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
      activeWhereSql.value.trim() || undefined,
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
  rowForm.value = rowToForm(
    defaultRow(selectedTable.value),
    selectedTable.value,
  );
  rowEditorOpen.value = true;
}

function editRow(row: Record<string, unknown>) {
  if (!selectedTable.value) return;
  originalRow.value = { ...row };
  rowForm.value = rowToForm(row, selectedTable.value);
  rowEditorOpen.value = true;
}

async function saveRow() {
  if (!connectionId.value || !selectedTableName.value || !selectedTable.value)
    return;

  const parsed = formToRow(selectedTable.value);

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
    rowForm.value = {};
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

function updateCell(
  row: Record<string, unknown>,
  columnName: string,
  value: string,
) {
  row[columnName] = parseCell(value);
}

function submitWhere() {
  activeWhereSql.value = whereSql.value.trim();
  page.value = 0;
  loadRows();
}

watch(selectedTableName, () => {
  page.value = 0;
  whereSql.value = "";
  activeWhereSql.value = "";
  rowForm.value = {};
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

    <div class="grid min-h-0 flex-1 xl:grid-cols-[250px_1fr]">
      <aside class="flex min-h-0 flex-col bg-default/30">
        <div class="shrink-0 p-2">
          <UInput
            v-model="tableSearch"
            icon="i-lucide-search"
            placeholder="Search tables"
            aria-label="Search tables by name"
            class="w-full"
          />
        </div>
        <div class="min-h-0 flex-1 overflow-y-auto">
          <p
            v-if="!tableItems.length"
            class="mx-2 rounded-md border border-default bg-default/20 px-3 py-2 text-sm text-muted"
          >
            No tables found.
          </p>
          <UTabs
            v-else
            v-model="selectedTableName"
            orientation="vertical"
            variant="pill"
            :content="false"
            :items="tableItems"
            class="w-full"
            :ui="{
              list: 'items-start bg-opacity-0 w-full',
              trigger: 'w-full',
            }"
          />
        </div>
      </aside>

      <section
        class="min-h-0 min-w-0 flex flex-col border-l border-default bg-default/30"
      >
        <div
          class="shrink-0 flex flex-wrap items-center justify-between gap-3 border-b border-default p-2"
        >
          <!-- <div class="min-w-0">
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
          </div> -->
          <form
            class="flex flex-wrap items-center gap-2"
            @submit.prevent="submitWhere"
          >
            <UFieldGroup>
              <UInput
                v-model="whereSql"
                placeholder="WHERE"
                class="w-32 sm:w-48"
                aria-label="WHERE query"
              />
              <UButton
                icon="i-lucide-search"
                color="neutral"
                variant="soft"
                type="submit"
                :loading="loadingRows"
              />
            </UFieldGroup>
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
              type="button"
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
              type="button"
              :disabled="!tablePage?.hasMore"
              aria-label="Next page"
              @click="
                page++;
                loadRows();
              "
            />
            <UButton icon="i-lucide-plus" type="button" @click="newRow"
              >New Row</UButton
            >
          </form>
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

    <USlideover
      v-model:open="rowEditorOpen"
      :title="originalRow ? 'Edit Row' : 'Create Row'"
      :description="selectedTableName"
      side="right"
      :ui="{
        content: 'sm:max-w-xl',
        body: 'min-h-0 flex-1',
        footer: 'shrink-0',
      }"
    >
      <template #body>
        <UForm
          id="row-editor-form"
          :state="rowForm"
          class="space-y-4"
          @submit="saveRow"
        >
          <UFormField
            v-for="column in selectedTable?.columns ?? []"
            :key="column.name"
            :label="column.name"
            :hint="column.type"
          >
            <UCheckbox
              v-if="isBooleanColumn(column)"
              :model-value="getBooleanField(column.name)"
              @update:model-value="setBooleanField(column.name, $event)"
            />
            <UInput
              v-else-if="isNumberColumn(column)"
              :model-value="getTextField(column.name)"
              class="w-full"
              type="number"
              @update:model-value="setTextField(column.name, $event)"
            />
            <UInput
              v-else
              :model-value="getTextField(column.name)"
              class="w-full"
              @update:model-value="setTextField(column.name, $event)"
            />
          </UFormField>
        </UForm>
      </template>

      <template #footer="{ close }">
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="close"
            >Cancel</UButton
          >
          <UButton
            icon="i-lucide-save"
            :loading="savingRow"
            type="submit"
            form="row-editor-form"
          >
            Save Row
          </UButton>
        </div>
      </template>
    </USlideover>
  </div>
</template>
