<script setup lang="ts">
import type { DropdownMenuItem, TableColumn, TabsItem } from "@nuxt/ui";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  createRow,
  exportTable,
  getSchema,
  getSelectedConnectionId,
  queryTable,
  removeRow,
  updateRow,
  type ColumnSummary,
  type ExportFormat,
  type Row,
  type SchemaSummary,
  type TablePage,
  type TableSummary,
} from "../lib/spacetime";
import { cellToInput, formatCell, isEditable, parseInput } from "../lib/cells";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";
import { dataTableUi } from "../lib/tableUi";
import { useConnections } from "../lib/connectionStore";
import ConnectionEmptyState from "../components/ConnectionEmptyState.vue";

const { canAccess, loadConnections, selectedConnection } = useConnections();
const toast = useToast();

const connectionId = ref<string | null>(getSelectedConnectionId());
const schema = ref<SchemaSummary | null>(null);
const selectedTableName = ref("");
const tablePage = ref<TablePage | null>(null);
const page = ref(0);
const pageSize = ref(25);
const loadingSchema = ref(false);
const loadingRows = ref(false);
const savingRow = ref(false);
const exporting = ref(false);
const error = ref("");
const status = ref("");
type RowFormValue = string | boolean;

const rowForm = ref<Record<string, RowFormValue>>({});
const originalRow = ref<Row | null>(null);
const rowEditorOpen = ref(false);
const pendingDelete = ref<Row | null>(null);
const whereSql = ref("");
const activeWhereSql = ref("");
const tableSearch = ref("");

// Unsaved inline edits, keyed by the row's index on the current page and then
// by column. The loaded rows themselves are never mutated, so they still
// identify the original record when the edit is saved.
const drafts = ref<Record<number, Record<string, string>>>({});

const readOnly = computed(() => selectedConnection.value?.readOnly ?? false);

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
    icon: table.scheduledReducer ? "i-lucide-timer" : "i-lucide-table",
  };
}

// Schedule tables lead each section; otherwise the schema's order is kept.
function sectionItems(matches: (table: TableSummary) => boolean) {
  return filteredTables.value
    .filter(matches)
    .sort(
      (a, b) => Number(Boolean(b.scheduledReducer)) - Number(Boolean(a.scheduledReducer)),
    )
    .map(toTabItem);
}

// Private tables are listed first, each group under its own heading. Groups
// with no matching tables are left out.
const tableSections = computed(() =>
  [
    { label: "Private", items: sectionItems(isPrivateTable) },
    { label: "Public", items: sectionItems((table) => !isPrivateTable(table)) },
  ].filter((section) => section.items.length),
);
// Sections start expanded; this tracks the ones the user has folded away.
const collapsedSections = ref<Record<string, boolean>>({});
const selectedTable = computed<TableSummary | null>(
  () =>
    tables.value.find((table) => table.name === selectedTableName.value) ??
    null,
);

// The schema knows the declared types (named structs, enums); the SQL response
// only knows the structural ones. Prefer the schema's view of each column.
const pageColumns = computed<ColumnSummary[]>(() => {
  const declared = selectedTable.value?.columns ?? [];
  const returned = tablePage.value?.columns ?? [];
  if (!returned.length) return declared;
  return returned.map(
    (column) => declared.find((known) => known.name === column.name) ?? column,
  );
});

const rangeLabel = computed(() => {
  const current = tablePage.value;
  if (!current) return "";
  if (!current.rows.length) return current.total ? "No rows on this page" : "No rows";
  const first = page.value * current.pageSize + 1;
  const last = page.value * current.pageSize + current.rows.length;
  const total =
    current.total == null ? "" : ` of ${current.total.toLocaleString()}`;
  return `Rows ${first.toLocaleString()}–${last.toLocaleString()}${total}`;
});

// Cells use separate borders so they stay attached to the sticky header. The
// last data column drops its right border because the pinned Actions column
// already draws one on its left.
const rowColumns = computed<TableColumn<Row>[]>(() => {
  const columns = pageColumns.value;

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

const exportItems = computed<DropdownMenuItem[]>(() =>
  (["csv", "json"] as ExportFormat[]).map((format) => ({
    label: `Export ${format.toUpperCase()}`,
    icon: format === "csv" ? "i-lucide-sheet" : "i-lucide-braces",
    onSelect: () => exportRows(format),
  })),
);

// Inline editing -----------------------------------------------------------

function draftText(index: number, row: Row, column: ColumnSummary) {
  return drafts.value[index]?.[column.name] ?? cellToInput(row[column.name], column);
}

function setDraft(index: number, row: Row, column: ColumnSummary, text: string) {
  const rowDraft = { ...(drafts.value[index] ?? {}) };
  if (text === cellToInput(row[column.name], column)) {
    delete rowDraft[column.name];
  } else {
    rowDraft[column.name] = text;
  }

  const next = { ...drafts.value };
  if (Object.keys(rowDraft).length) next[index] = rowDraft;
  else delete next[index];
  drafts.value = next;
}

function isDirty(index: number, columnName?: string) {
  const rowDraft = drafts.value[index];
  if (!rowDraft) return false;
  return columnName ? columnName in rowDraft : true;
}

function discardDraft(index: number) {
  const next = { ...drafts.value };
  delete next[index];
  drafts.value = next;
}

async function saveDraft(index: number, row: Row) {
  const rowDraft = drafts.value[index];
  if (!rowDraft) return;

  const changes = Object.fromEntries(
    pageColumns.value
      .filter((column) => column.name in rowDraft)
      .map((column) => [column.name, parseInput(rowDraft[column.name], column)]),
  );
  await persist(() => updateRow(connectionId.value!, selectedTableName.value, row, changes), "Row updated.");
}

// Slide-over editor ------------------------------------------------------

function rowToForm(row: Row, columns: ColumnSummary[]) {
  return Object.fromEntries(
    columns.map((column) => [
      column.name,
      column.kind === "bool"
        ? row[column.name] === true || row[column.name] === "true"
        : cellToInput(row[column.name], column),
    ]),
  );
}

function formText(column: ColumnSummary) {
  const value = rowForm.value[column.name];
  return typeof value === "boolean" ? String(value) : (value ?? "");
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

function newRow() {
  if (!selectedTable.value) return;
  originalRow.value = null;
  rowForm.value = Object.fromEntries(
    selectedTable.value.columns.map((column) => [
      column.name,
      column.kind === "bool" ? false : "",
    ]),
  );
  rowEditorOpen.value = true;
}

function editRow(row: Row) {
  originalRow.value = row;
  rowForm.value = rowToForm(row, pageColumns.value);
  rowEditorOpen.value = true;
}

async function saveRow() {
  if (!connectionId.value || !selectedTable.value) return;
  const editable = (originalRow.value ? pageColumns.value : selectedTable.value.columns).filter(isEditable);

  if (originalRow.value) {
    const original = originalRow.value;
    const initial = rowToForm(original, pageColumns.value);
    const changes = Object.fromEntries(
      editable
        .filter((column) => rowForm.value[column.name] !== initial[column.name])
        .map((column) => [column.name, parseInput(formText(column), column)]),
    );
    await persist(
      () => updateRow(connectionId.value!, selectedTableName.value, original, changes),
      "Row updated.",
    );
  } else {
    // Leave blank non-string fields out so the server can apply defaults
    // (such as auto_inc) or report exactly which column is missing.
    const values = Object.fromEntries(
      editable
        .filter((column) => column.kind === "string" || column.kind === "bool" || formText(column).trim() !== "")
        .map((column) => [column.name, parseInput(formText(column), column)]),
    );
    await persist(
      () => createRow(connectionId.value!, selectedTableName.value, values),
      "Row inserted.",
    );
  }

  if (!error.value) {
    rowEditorOpen.value = false;
    originalRow.value = null;
  }
}

async function confirmDelete() {
  const row = pendingDelete.value;
  if (!row || !connectionId.value) return;
  await persist(() => removeRow(connectionId.value!, selectedTableName.value, row), "Row deleted.");
  pendingDelete.value = null;
}

// Runs a write, then reloads the page so the table shows what the server has.
async function persist(action: () => Promise<void>, success: string) {
  if (!connectionId.value || !selectedTableName.value) return;

  savingRow.value = true;
  error.value = "";
  status.value = "";

  try {
    await action();
    status.value = success;
    await loadRows();
  } catch (err) {
    error.value = String(err);
  } finally {
    savingRow.value = false;
  }
}

// Loading ----------------------------------------------------------------

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

  try {
    tablePage.value = await queryTable(
      connectionId.value,
      selectedTableName.value,
      page.value,
      pageSize.value,
      activeWhereSql.value.trim() || undefined,
    );
    drafts.value = {};
  } catch (err) {
    error.value = String(err);
  } finally {
    loadingRows.value = false;
  }
}

async function exportRows(format: ExportFormat) {
  if (!connectionId.value || !selectedTableName.value) return;

  exporting.value = true;
  try {
    const path = await exportTable(
      connectionId.value,
      selectedTableName.value,
      format,
      activeWhereSql.value.trim() || undefined,
    );
    if (path) {
      toast.add({ title: "Export saved", description: path, color: "success", icon: "i-lucide-check" });
    }
  } catch (err) {
    toast.add({ title: "Export failed", description: String(err), color: "error" });
  } finally {
    exporting.value = false;
  }
}

function goToPage(next: number) {
  page.value = next;
  loadRows();
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
  status.value = "";
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
      close
      @update:open="error = ''"
    />
    <UAlert
      v-if="status"
      color="success"
      variant="subtle"
      :description="status"
      class="shrink-0"
      close
      @update:open="status = ''"
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
            {{ loadingSchema ? "Loading tables..." : "No tables found." }}
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
          <!-- basis-40 lets the controls share this line until the info would be
               squeezed below 10rem; until then a long reducer name truncates. -->
          <div class="flex min-w-0 flex-1 basis-40 items-center gap-2 px-1 text-xs text-muted">
            <UBadge
              v-if="selectedTable?.scheduledReducer"
              color="neutral"
              variant="subtle"
              icon="i-lucide-timer"
              size="sm"
              class="min-w-0"
              :title="`Rows in this table schedule calls to ${selectedTable.scheduledReducer}`"
            >
              <span class="truncate">
                Schedules <span class="font-mono">{{ selectedTable.scheduledReducer }}</span>
              </span>
            </UBadge>
            <span v-if="tablePage" class="shrink-0 whitespace-nowrap">{{ rangeLabel }}</span>
            <UBadge v-if="readOnly" color="neutral" variant="subtle" icon="i-lucide-lock" size="sm" class="shrink-0">
              Read-only
            </UBadge>
          </div>
          <!-- ml-auto keeps the controls right-aligned when the toolbar wraps. -->
          <form
            class="ml-auto flex flex-wrap items-center gap-2"
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
              @click="goToPage(page - 1)"
            />
            <UButton
              icon="i-lucide-chevron-right"
              color="neutral"
              variant="soft"
              type="button"
              :disabled="!tablePage?.hasMore"
              aria-label="Next page"
              @click="goToPage(page + 1)"
            />
            <UDropdownMenu :items="exportItems">
              <UButton
                icon="i-lucide-download"
                color="neutral"
                variant="soft"
                type="button"
                :loading="exporting"
                :disabled="!selectedTableName"
                aria-label="Export rows"
              />
            </UDropdownMenu>
            <UButton
              v-if="!readOnly"
              icon="i-lucide-plus"
              type="button"
              :disabled="!selectedTable"
              @click="newRow"
              >New Row</UButton
            >
          </form>
        </div>

        <UAlert
          v-if="tablePage?.scanLimit"
          color="warning"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          :description="`Paging stops after the first ${tablePage.scanLimit.toLocaleString()} rows. Add a WHERE filter to narrow the table.`"
          class="m-2 shrink-0"
        />

        <UTable
          :data="tablePage?.rows ?? []"
          :columns="rowColumns"
          :loading="loadingRows"
          sticky="header"
          :column-pinning="{ right: ['actions'] }"
          class="min-h-0 flex-1"
          :ui="{ ...dataTableUi, empty: loadingRows ? 'hidden' : dataTableUi.empty }"
        >
          <template
            v-for="column in pageColumns"
            :key="column.name"
            #[`${column.name}-header`]
          >
            <span class="inline-flex items-center gap-1">
              <UIcon
                v-if="selectedTable?.primaryKey.includes(column.name)"
                name="i-lucide-key-round"
                class="size-3 text-warning"
                title="Primary key"
              />
              {{ column.name }}
              <span class="font-normal">({{ column.type }})</span>
            </span>
          </template>

          <template
            v-for="column in pageColumns"
            :key="column.name"
            #[`${column.name}-cell`]="{ row }"
          >
            <template v-if="isEditable(column) && !readOnly">
              <UCheckbox
                v-if="column.kind === 'bool'"
                class="px-2 py-1"
                :model-value="draftText(row.index, row.original, column) === 'true'"
                @update:model-value="setDraft(row.index, row.original, column, String($event === true))"
              />
              <input
                v-else
                class="w-full rounded border bg-transparent px-2 py-1 text-highlighted outline-none focus:border-primary focus:bg-default"
                :class="isDirty(row.index, column.name) ? 'border-warning/60 bg-warning/5' : 'border-transparent'"
                :value="draftText(row.index, row.original, column)"
                @input="setDraft(row.index, row.original, column, ($event.target as HTMLInputElement).value)"
                @keydown.enter="saveDraft(row.index, row.original)"
                @keydown.escape="discardDraft(row.index)"
              />
            </template>
            <span
              v-else
              class="block max-w-md truncate px-2 py-1 text-highlighted"
              :title="formatCell(row.original[column.name], column.kind)"
            >
              {{ formatCell(row.original[column.name], column.kind) }}
            </span>
          </template>

          <template #actions-cell="{ row }">
            <template v-if="!readOnly">
              <UButton
                icon="i-lucide-save"
                size="xs"
                variant="ghost"
                aria-label="Save row"
                title="Save changes (Enter)"
                :disabled="!isDirty(row.index)"
                :loading="savingRow && isDirty(row.index)"
                @click="saveDraft(row.index, row.original)"
              />
              <UButton
                icon="i-lucide-undo-2"
                size="xs"
                color="neutral"
                variant="ghost"
                aria-label="Discard changes"
                title="Discard changes (Esc)"
                :disabled="!isDirty(row.index)"
                @click="discardDraft(row.index)"
              />
            </template>
            <UButton
              :icon="readOnly ? 'i-lucide-eye' : 'i-lucide-file-pen-line'"
              size="xs"
              color="neutral"
              variant="ghost"
              :aria-label="readOnly ? 'View row' : 'Edit row'"
              @click="editRow(row.original)"
            />
            <UButton
              v-if="!readOnly"
              icon="i-lucide-trash-2"
              size="xs"
              color="error"
              variant="ghost"
              aria-label="Delete row"
              @click="pendingDelete = row.original"
            />
          </template>

          <template #empty>
            {{ activeWhereSql ? "No rows match this filter." : "This table is empty." }}
          </template>
        </UTable>
      </section>
    </div>

    <USlideover
      v-model:open="rowEditorOpen"
      :title="readOnly ? 'Row' : originalRow ? 'Edit Row' : 'Create Row'"
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
            v-for="column in originalRow ? pageColumns : (selectedTable?.columns ?? [])"
            :key="column.name"
            :label="column.name"
            :hint="column.type"
            :help="!isEditable(column) && !readOnly ? 'This type can\'t be written with SQL. Use a reducer to change it.' : undefined"
          >
            <UCheckbox
              v-if="column.kind === 'bool'"
              :model-value="getBooleanField(column.name)"
              :disabled="readOnly"
              @update:model-value="setBooleanField(column.name, $event)"
            />
            <UTextarea
              v-else-if="!isEditable(column)"
              :model-value="originalRow ? formatCell(originalRow[column.name], column.kind) : ''"
              class="w-full font-mono"
              autoresize
              :rows="1"
              disabled
            />
            <UInput
              v-else
              :model-value="getTextField(column.name)"
              class="w-full"
              :inputmode="column.kind === 'integer' || column.kind === 'float' ? 'decimal' : undefined"
              :placeholder="column.kind === 'identity' ? '0x…' : undefined"
              :disabled="readOnly"
              @update:model-value="setTextField(column.name, $event)"
            />
          </UFormField>
        </UForm>
      </template>

      <template #footer="{ close }">
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="close"
            >{{ readOnly ? "Close" : "Cancel" }}</UButton
          >
          <UButton
            v-if="!readOnly"
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

    <UModal
      :open="pendingDelete !== null"
      title="Delete row"
      :description="`Permanently delete this row from ${selectedTableName}? This can't be undone.`"
      @update:open="pendingDelete = null"
    >
      <template #body>
        <pre
          class="max-h-60 overflow-auto rounded-md border border-default bg-default/40 p-3 text-xs text-highlighted"
        >{{ JSON.stringify(pendingDelete, null, 2) }}</pre>
      </template>
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="() => { pendingDelete = null }">
            Cancel
          </UButton>
          <UButton color="error" :loading="savingRow" @click="confirmDelete">Delete</UButton>
        </div>
      </template>
    </UModal>
  </div>
</template>
