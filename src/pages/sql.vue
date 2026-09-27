<script setup lang="ts">
import type { TableColumn } from "@nuxt/ui";
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  executeSql,
  exportRows,
  getSelectedConnectionId,
  type ExportFormat,
  type SqlStatementResult,
} from "../lib/spacetime";
import { formatCell } from "../lib/cells";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";
import { dataTableUi } from "../lib/tableUi";
import { useConnections } from "../lib/connectionStore";

const HISTORY_KEY = "spacetime-studio:sql-history";
const HISTORY_LIMIT = 25;

const { selectedConnection } = useConnections();
const toast = useToast();

const query = ref("SELECT * FROM ");
const running = ref(false);
const error = ref("");
const results = ref<SqlStatementResult[]>([]);
const ranAt = ref<string>("");
const activeResult = ref("0");
const history = ref<string[]>(loadHistory());

const readOnly = computed(() => selectedConnection.value?.readOnly ?? false);

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
    header: `${column.name} (${column.type})`,
    accessorFn: (row) => formatCell(row[index], column.kind),
    meta: { class: { th: "whitespace-nowrap border-r border-default", td: "border-r border-default/60 font-mono" } },
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
  error.value = "";

  try {
    const response = await executeSql(connectionId, sql);
    results.value = response.statements;
    activeResult.value = "0";
    ranAt.value = new Date().toLocaleTimeString();
    remember(sql);
  } catch (err) {
    error.value = String(err);
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
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-3 p-4">
    <div class="shrink-0 space-y-2">
      <UTextarea
        v-model="query"
        :rows="6"
        autoresize
        :maxrows="16"
        class="w-full"
        :ui="{ base: 'font-mono text-sm' }"
        placeholder="SELECT * FROM my_table WHERE id = 1"
        aria-label="SQL query"
        @keydown="onKeydown"
      />
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="text-xs text-muted">
          <UIcon v-if="readOnly" name="i-lucide-lock" class="mr-1 size-3 align-[-2px]" />
          {{
            readOnly
              ? "Read-only connection: only SELECT, SHOW and EXPLAIN statements will run."
              : "Writes (INSERT, UPDATE, DELETE) run immediately against the database."
          }}
          Separate statements with <code>;</code>.
        </p>
        <div class="flex items-center gap-2">
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
        </div>
      </div>
    </div>

    <UAlert
      v-if="error"
      color="error"
      variant="subtle"
      :description="error"
      class="shrink-0"
    />

    <div
      v-if="results.length"
      class="flex min-h-0 flex-1 flex-col rounded-md border border-default bg-default/30"
    >
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
      <UTable
        v-else
        :data="current?.rows ?? []"
        :columns="tableColumns"
        sticky="header"
        class="min-h-0 flex-1"
        :ui="dataTableUi"
      >
        <template #empty>No rows returned.</template>
      </UTable>
    </div>

    <div
      v-else-if="!error"
      class="flex flex-1 items-center justify-center rounded-md border border-dashed border-default text-sm text-muted"
    >
      Run a query to see results here.
    </div>
  </div>
</template>
