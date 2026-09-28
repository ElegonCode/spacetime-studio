<script setup lang="ts">
import type { TableColumn } from "@nuxt/ui";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useToast } from "@nuxt/ui/composables";
import {
  executeSql,
  exportRows,
  getSchema,
  getSelectedConnectionId,
  type ExportFormat,
  type SqlStatementResult,
  type TableSummary,
} from "../lib/spacetime";
import { formatCell } from "../lib/cells";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";
import { dataTableUi, selectedCellUi } from "../lib/tableUi";
import { useConnections } from "../lib/connectionStore";
import SqlCodeEditor from "../components/SqlCodeEditor.vue";

const HISTORY_KEY = "spacetime-studio:sql-history";
const HISTORY_LIMIT = 25;

const { selectedConnection } = useConnections();
const toast = useToast();

const query = ref("SELECT * FROM ");
const running = ref(false);
const results = ref<SqlStatementResult[]>([]);
const ranAt = ref<string>("");
const activeResult = ref("0");
const history = ref<string[]>(loadHistory());
const schemaTables = ref<TableSummary[]>([]);

const readOnly = computed(() => selectedConnection.value?.readOnly ?? false);
const sqlHelp = computed(() =>
  `${readOnly.value ? "Read-only connections allow SELECT, SHOW, and EXPLAIN." : "INSERT, UPDATE, and DELETE run immediately."} SpacetimeDB SQL has a limited dialect. Use Ctrl+Space for supported keywords, tables, and columns. Separate statements with semicolons.`,
);

function loadHistory(): string[] {
  try {
    const stored = JSON.parse(localStorage.getItem(HISTORY_KEY) ?? "[]");
    return Array.isArray(stored) ? stored.filter((item) => typeof item === "string") : [];
  } catch {
    return [];
  }
}

function remember(sql: string) {
  history.value = [sql, ...history.value.filter((item) => item !== sql)].slice(0, HISTORY_LIMIT);
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(history.value));
  } catch {
    // History is a convenience; losing it is fine.
  }
}

const historyItems = computed(() =>
  history.value.map((sql) => ({
    label: sql.length > 80 ? `${sql.slice(0, 80)}…` : sql,
    onSelect: () => {
      query.value = sql;
    },
  })),
);

const resultTabs = computed(() =>
  results.value.map((result, index) => ({
    label: results.value.length > 1 ? `Statement ${index + 1}` : "Result",
    value: String(index),
    badge: result.rows.length,
  })),
);

const current = computed<SqlStatementResult | null>(
  () => results.value[Number(activeResult.value)] ?? null,
);

// SQL results are positional arrays; key each cell by column index so
// duplicate or empty column names still render.
const tableColumns = computed<TableColumn<unknown[]>[]>(() =>
  (current.value?.columns ?? []).map((column, index) => ({
    id: `c${index}`,
    header: column.name,
    accessorFn: (row) => formatCell(row[index], column.kind),
    meta: {
      class: {
        th: "whitespace-nowrap border-r border-default",
        td: "min-w-40 border-r border-default/60 align-top",
      },
    },
  })),
);

function formatDuration(micros?: number | null) {
  if (micros == null) return "";
  return micros < 1000 ? `${micros} µs` : `${(micros / 1000).toFixed(1)} ms`;
}

async function run() {
  const connectionId = getSelectedConnectionId();
  const sql = query.value.trim();
  if (!connectionId || !sql || running.value) return;

  running.value = true;

  try {
    const response = await executeSql(connectionId, sql);
    results.value = response.statements;
    activeResult.value = "0";
    ranAt.value = new Date().toLocaleTimeString();
    remember(sql);
  } catch (err) {
    toast.add({
      title: "SQL query failed",
      description: String(err),
      color: "error",
      icon: "i-lucide-circle-alert",
    });
  } finally {
    running.value = false;
  }
}

async function exportCurrent(format: ExportFormat) {
  if (!current.value) return;
  try {
    const path = await exportRows("query", format, current.value.columns, current.value.rows);
    if (path) {
      toast.add({ title: "Export saved", description: path, color: "success", icon: "i-lucide-check" });
    }
  } catch (err) {
    toast.add({ title: "Export failed", description: String(err), color: "error" });
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    run();
  }
}

onMounted(() => setPageRefreshHandler(run));
onUnmounted(() => clearPageRefreshHandler(run));

watch(
  () => selectedConnection.value?.id,
  async (connectionId) => {
    schemaTables.value = [];
    if (!connectionId) return;
    try {
      const schema = await getSchema(connectionId);
      if (selectedConnection.value?.id === connectionId) {
        schemaTables.value = schema.tables;
      }
    } catch {
      // Keep the editor usable even if schema introspection is unavailable.
    }
  },
  { immediate: true },
);
</script>

<template>
  <div class="grid h-full min-h-0 grid-rows-2">
    <section class="min-h-0 border-b border-default">
      <SqlCodeEditor
        v-model="query"
        :tables="schemaTables"
        @keydown="onKeydown"
      />
    </section>

    <section class="flex min-h-0 flex-col">
      <div v-if="results.length" class="flex min-h-0 flex-1 flex-col">
      <div class="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-default p-2">
        <UTabs
          v-if="results.length > 1"
          v-model="activeResult"
          :items="resultTabs"
          :content="false"
          size="xs"
          variant="link"
        />
        <span class="px-1 text-xs text-muted">
          {{ current?.rows.length.toLocaleString() }} row(s)
          <template v-if="current?.durationMicros != null">
            &middot; {{ formatDuration(current.durationMicros) }}
          </template>
          &middot; ran at {{ ranAt }}
        </span>
        <div class="flex gap-1">
          <UButton
            icon="i-lucide-sheet"
            size="xs"
            color="neutral"
            variant="soft"
            :disabled="!current?.columns.length"
            @click="exportCurrent('csv')"
            >CSV</UButton
          >
          <UButton
            icon="i-lucide-braces"
            size="xs"
            color="neutral"
            variant="soft"
            :disabled="!current?.columns.length"
            @click="exportCurrent('json')"
            >JSON</UButton
          >
        </div>
      </div>

      <p v-if="current && !current.columns.length" class="p-3 text-sm text-muted">
        Statement ran successfully and returned no result set.
      </p>
      <div v-else class="min-h-0 min-w-0 flex-1 overflow-auto">
      <UTable
        :data="current?.rows ?? []"
        :columns="tableColumns"
        sticky="header"
        class="min-w-max"
        :ui="dataTableUi"
      >
        <template
          v-for="(column, index) in current?.columns ?? []"
          :key="`c${index}`"
          #[`c${index}-header`]
        >
          <span class="inline-flex items-center gap-1">
            {{ column.name }}
            <span class="font-normal">({{ column.type }})</span>
          </span>
        </template>
        <template
          v-for="(column, index) in current?.columns ?? []"
          :key="`c${index}-cell`"
          #[`c${index}-cell`]="{ row }"
        >
          <div :tabindex="0" :class="selectedCellUi" class="max-w-md">
            <span
              class="block truncate text-highlighted"
              :title="formatCell(row.original[index], column.kind)"
            >
              {{ formatCell(row.original[index], column.kind) }}
            </span>
          </div>
        </template>
        <template #empty>No rows returned.</template>
      </UTable>
      </div>
      </div>

      <div
        v-else
        class="flex min-h-0 flex-1 items-center justify-center text-sm text-muted"
      >
        Run a query to see results here.
      </div>
    </section>

    <Teleport to="#page-header-actions">
      <UTooltip :text="sqlHelp">
        <UButton
          icon="i-lucide-circle-help"
          color="neutral"
          variant="ghost"
          aria-label="SQL editor help"
        />
      </UTooltip>
      <UDropdownMenu :items="historyItems" :content="{ align: 'end' }">
        <UButton
          icon="i-lucide-history"
          color="neutral"
          variant="soft"
          :disabled="!history.length"
          aria-label="Query history"
          title="Query history"
        />
      </UDropdownMenu>
      <UButton icon="i-lucide-play" :loading="running" @click="run">
        Run
        <UKbd value="ctrl" size="sm" class="ml-1" /><UKbd value="enter" size="sm" />
      </UButton>
    </Teleport>
  </div>
</template>
