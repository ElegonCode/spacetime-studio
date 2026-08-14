<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { getLogs, getSelectedConnectionId } from "../lib/spacetime";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";

type LogLevel = "error" | "warning" | "info" | "debug";

interface ParsedLog {
  id: number;
  level: LogLevel;
  message: string;
  timestamp: string;
  source: string;
}

const LEVEL_STYLES: Record<
  LogLevel,
  { color: "error" | "warning" | "info" | "neutral"; icon: string }
> = {
  error: { color: "error", icon: "i-lucide-circle-x" },
  warning: { color: "warning", icon: "i-lucide-triangle-alert" },
  info: { color: "info", icon: "i-lucide-info" },
  debug: { color: "neutral", icon: "i-lucide-bug" },
};

function normalizeLevel(raw: unknown): LogLevel {
  const value = String(raw ?? "").toLowerCase();
  if (value === "error" || value === "panic" || value === "fatal") {
    return "error";
  }
  if (value === "warn" || value === "warning") return "warning";
  if (value === "debug" || value === "trace") return "debug";
  return "info";
}

function formatTimestamp(raw: unknown): string {
  let micros: number | undefined;
  if (typeof raw === "number") {
    micros = raw;
  } else if (raw && typeof raw === "object") {
    const nested = (raw as Record<string, unknown>)[
      "__timestamp_micros_since_unix_epoch__"
    ];
    if (typeof nested === "number") micros = nested;
  }
  if (micros === undefined) return "";
  const date = new Date(micros / 1000);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString();
}

function parseLogLine(line: string, id: number): ParsedLog {
  try {
    const entry = JSON.parse(line) as Record<string, unknown>;
    const source =
      entry.filename && entry.line_number !== undefined
        ? `${entry.filename}:${entry.line_number}`
        : String(entry.target ?? "");
    return {
      id,
      level: normalizeLevel(entry.level),
      message: String(entry.message ?? line),
      timestamp: formatTimestamp(entry.ts),
      source,
    };
  } catch {
    return { id, level: "info", message: line, timestamp: "", source: "" };
  }
}

const parsedLogs = computed<ParsedLog[]>(() => {
  return logs.value
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .map((line, index) => parseLogLine(line, index));
});

const logs = ref("");
const numLines = ref(200);
const loading = ref(false);
const error = ref("");
const autoRefresh = ref(true);
const pollIntervalMs = ref(2000);
let pollTimer: ReturnType<typeof setInterval> | undefined;
let requestInFlight = false;

async function load(showLoading = true) {
  if (requestInFlight) return;

  const connectionId = getSelectedConnectionId();
  if (!connectionId) {
    error.value = "Select or create a connection first.";
    return;
  }

  requestInFlight = true;
  if (showLoading) loading.value = true;
  error.value = "";

  try {
    logs.value = await getLogs(connectionId, numLines.value);
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
    requestInFlight = false;
  }
}

function startPolling() {
  if (pollTimer) clearInterval(pollTimer);
  if (!autoRefresh.value) return;

  pollTimer = setInterval(() => {
    load(false);
  }, pollIntervalMs.value);
}

watch(autoRefresh, startPolling);
watch(numLines, () => load());

onMounted(() => {
  load();
  startPolling();
  setPageRefreshHandler(load);
});

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer);
  clearPageRefreshHandler(load);
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-4 p-4">
    <div class="shrink-0 flex justify-end">
      <div class="flex flex-wrap items-center gap-2">
        <UCheckbox v-model="autoRefresh" label="Live" />
        <UInput
          v-model.number="numLines"
          type="number"
          min="1"
          max="1000"
          class="w-28"
        />
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
      class="min-h-0 flex-1 overflow-auto rounded-lg border border-default p-2"
    >
      <div v-if="parsedLogs.length" class="flex flex-col gap-2">
        <UAlert
          v-for="log in parsedLogs"
          :key="log.id"
          :color="LEVEL_STYLES[log.level].color"
          :icon="LEVEL_STYLES[log.level].icon"
          variant="subtle"
          :description="log.message"
          :ui="{ description: 'whitespace-pre-wrap break-words font-mono text-xs' }"
        >
          <template v-if="log.timestamp || log.source" #title>
            <span class="flex flex-wrap items-center gap-2 text-xs font-normal">
              <span v-if="log.timestamp" class="text-muted">{{
                log.timestamp
              }}</span>
              <span v-if="log.source" class="text-dimmed font-mono">{{
                log.source
              }}</span>
            </span>
          </template>
        </UAlert>
      </div>
      <p v-else class="p-2 text-sm text-muted">No logs loaded.</p>
    </div>
  </div>
</template>
