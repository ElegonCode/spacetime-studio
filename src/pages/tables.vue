<script setup lang="ts">
import type { TableColumn, TabsItem } from "@nuxt/ui";
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
import { dataTableUi } from "../lib/tableUi";
import { useConnections } from "../lib/connectionStore";
import ConnectionEmptyState from "../components/ConnectionEmptyState.vue";

const { canAccess, loadConnections } = useConnections();

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
function isPrivateTable(table: TableSummary) {
  return table.access.toLowerCase() === "private";
}

function toTabItem(table: TableSummary): TabsItem {

  return {
    label: table.name,
    value: table.name,
    icon: "i-lucide-table"
  };
}

// Private tables are listed first, each group under its own heading. Groups
// with no matching tables are left out.
const tableSections = computed(() =>
  [
    {
      label: "Private",
      items: filteredTables.value.filter(isPrivateTable).map(toTabItem),
    },
    {
      label: "Public",
      items: filteredTables.value
        .filter((table) => !isPrivateTable(table))
        .map(toTabItem),
    },
  ].filter((section) => section.items.length),
);
// Sections start expanded; this tracks the ones the user has folded away.
const collapsedSections = ref<Record<string, boolean>>({});
const selectedTable = computed<TableSummary | null>(
  () =>
    tables.value.find((table) => table.name === selectedTableName.value) ??
    null,
);

type Row = Record<string, unknown>;

// Cells use separate borders so they stay attached to the sticky header. The
// last data column drops its right border because the pinned Actions column
// already draws one on its left.
const rowColumns = computed<TableColumn<Row>[]>(() => {
  const columns = tablePage.value?.columns ?? [];

  return [
    ...columns.map<TableColumn<Row>>((column, index) => {
      const edge = index === columns.length - 1 ? "" : "border-r";

      return {
        id: column.name,
        accessorFn: (row) => row[column.name],
        meta: {
          class: {
            th: `whitespace-nowrap ${edge} border-default`,
            td: `min-w-40 ${edge} border-default/60 align-top`,
          },
        },
      };
    }),
    {
      id: "actions",
      header: "Actions",
      meta: {
        class: {
          th: "w-28 text-right",
          td: "whitespace-nowrap text-right align-top",
        },
      },
    },
  ];
});

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

function clearSchema() {
  schema.value = null;
  tablePage.value = null;
  selectedTableName.value = "";
  error.value = "";
  status.value = "";
}

async function loadSchema() {
  connectionId.value = getSelectedConnectionId();
  // Without a usable connection the page stays empty rather than erroring.
  if (!connectionId.value || !canAccess.value) {
    clearSchema();
    return;
  }

  loadingSchema.value = true;
  error.value = "";

  try {
    const loaded = await getSchema(connectionId.value);
    schema.value = loaded;
    // Keep the open table across refreshes, but fall back to the first one
    // when it does not exist in this (possibly different) database.
    const keep = loaded.tables.some(
      (table) => table.name === selectedTableName.value,
    );
    const previous = selectedTableName.value;
    selectedTableName.value = keep
      ? previous
      : (loaded.tables[0]?.name ?? "");
    // The watcher on selectedTableName loads rows when the name changes.
    if (selectedTableName.value && selectedTableName.value === previous)
      await loadRows();
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

// With no usable connection there is no schema to reload, so the refresh button
// re-checks the saved connections instead.
async function refresh() {
  if (canAccess.value) {
    await loadSchema();
  } else {
    await loadConnections().catch(() => {});
  }
}

watch(canAccess, () => loadSchema());

onMounted(() => {
  loadSchema();
  setPageRefreshHandler(refresh);
});

onUnmounted(() => {
  clearPageRefreshHandler(refresh);
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
            v-if="!tableSections.length"
            class="mx-2 rounded-md border border-default bg-default/20 px-3 py-2 text-sm text-muted"
          >
            No tables found.
          </p>
          <UCollapsible
            v-for="section in tableSections"
            :key="section.label"
            :open="!collapsedSections[section.label]"
            class="mb-3"
            @update:open="collapsedSections[section.label] = !$event"
          >
            <button
              type="button"
              class="group flex w-full items-center gap-1 px-3 pt-1 pb-1 text-xs font-medium text-muted select-none hover:text-highlighted"
            >
              <UIcon
                name="i-lucide-chevron-down"
                class="size-3.5 transition-transform group-data-[state=closed]:-rotate-90"
              />
              {{ section.label }}
              <span class="font-normal text-dimmed">
                ({{ section.items.length }})
              </span>
            </button>

            <template #content>
              <UTabs
                v-model="selectedTableName"
                orientation="vertical"
                variant="pill"
                :content="false"
                :items="section.items"
                class="w-full"
                :ui="{
                  list: 'items-start bg-opacity-0 w-full',
                  trigger: 'w-full',
                  // Each section is its own tab list, so hide the pill in the
                  // ones that do not hold the selected table.
                  indicator: section.items.some(
                    (item) => item.value === selectedTableName,
                  )
                    ? undefined
                    : 'hidden',
                }"
              />
            </template>
          </UCollapsible>
        </div>
      </aside>

      <section
        v-if="!canAccess"
        class="min-h-0 min-w-0 border-l border-default bg-default/30"
      >
        <ConnectionEmptyState />
      </section>

      <section
        v-else
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

        <UTable
          :data="tablePage?.rows ?? []"
          :columns="rowColumns"
          sticky="header"
          :column-pinning="{ right: ['actions'] }"
          class="min-h-0 flex-1"
          :ui="{ ...dataTableUi, empty: 'hidden' }"
        >
          <template
            v-for="column in tablePage?.columns ?? []"
            :key="column.name"
            #[`${column.name}-header`]
          >
            {{ column.name }}
            <span class="font-normal">({{ column.type }})</span>
          </template>

          <template
            v-for="column in tablePage?.columns ?? []"
            :key="column.name"
            #[`${column.name}-cell`]="{ row }"
          >
            <input
              class="w-full rounded border border-transparent bg-transparent px-2 py-1 text-highlighted outline-none focus:border-primary focus:bg-default"
              :value="stringifyCell(row.original[column.name])"
              @change="
                updateCell(
                  row.original,
                  column.name,
                  ($event.target as HTMLInputElement).value,
                )
              "
            />
          </template>

          <template #actions-cell="{ row }">
            <UButton
              icon="i-lucide-save"
              size="xs"
              variant="ghost"
              aria-label="Save row"
              @click="
                editRow(row.original);
                saveRow();
              "
            />
            <UButton
              icon="i-lucide-file-pen-line"
              size="xs"
              color="neutral"
              variant="ghost"
              aria-label="Edit JSON"
              @click="editRow(row.original)"
            />
            <UButton
              icon="i-lucide-trash-2"
              size="xs"
              color="error"
              variant="ghost"
              aria-label="Delete row"
              @click="deleteRow(row.original)"
            />
          </template>
        </UTable>
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
