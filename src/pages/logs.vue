<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { getSelectedConnectionId, streamLogs, type LogEvent } from "../lib/spacetime";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";

type LogLevel = "error" | "warning" | "info" | "debug";
type StreamStatus = "idle" | "connecting" | "live" | "ended" | "error";

interface ParsedLog {
  id: number;
  level: LogLevel;
  message: string;
  timestamp: string;
  source: string;
}

// Older lines are dropped past this so a chatty module can't exhaust memory.
const MAX_ENTRIES = 5000;

const LEVEL_STYLES: Record<LogLevel, { text: string; icon: string }> = {
  error: { text: "text-error", icon: "i-lucide-circle-x" },
  warning: { text: "text-warning", icon: "i-lucide-triangle-alert" },
  info: { text: "text-info", icon: "i-lucide-info" },
  debug: { text: "text-dimmed", icon: "i-lucide-bug" },
};

const LEVEL_FILTERS = [
  { label: "All levels", value: "all" },
  { label: "Errors", value: "error" },
  { label: "Warnings+", value: "warning" },
  { label: "Info+", value: "info" },
];
const LEVEL_RANK: Record<LogLevel, number> = { debug: 0, info: 1, warning: 2, error: 3 };

const STATUS_BADGES: Record<StreamStatus, { label: string; color: "success" | "warning" | "error" | "neutral" }> = {
  idle: { label: "Idle", color: "neutral" },
  connecting: { label: "Connecting", color: "warning" },
  live: { label: "Live", color: "success" },
  ended: { label: "Loaded", color: "neutral" },
  error: { label: "Disconnected", color: "error" },
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

let nextId = 0;

function parseLogLine(line: string): ParsedLog {
  const id = nextId++;
  try {
    const entry = JSON.parse(line) as Record<string, unknown>;
    const source =
      entry.filename && entry.line_number !== undefined
        ? `${entry.filename}:${entry.line_number}`
        : String(entry.target ?? entry.function ?? "");
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

const entries = ref<ParsedLog[]>([]);
const numLines = ref(200);
const live = ref(true);
const autoScroll = ref(true);
const search = ref("");
const levelFilter = ref("all");
const streamStatus = ref<StreamStatus>("idle");
const error = ref("");
const scroller = ref<HTMLElement | null>(null);

const visibleEntries = computed(() => {
  const query = search.value.trim().toLowerCase();
  const minimum = levelFilter.value === "all" ? -1 : LEVEL_RANK[levelFilter.value as LogLevel];
  return entries.value.filter(
    (entry) =>
      LEVEL_RANK[entry.level] >= minimum &&
      (!query ||
        entry.message.toLowerCase().includes(query) ||
        entry.source.toLowerCase().includes(query)),
  );
});

let stopStream: (() => Promise<void>) | null = null;
// Guards against events from a stream that was replaced by a newer one.
let generation = 0;

async function stop() {
  generation++;
  const stopping = stopStream;
  stopStream = null;
  await stopping?.().catch(() => {});
}

function append(lines: string[]) {
  const next = entries.value.concat(lines.map(parseLogLine));
  entries.value = next.length > MAX_ENTRIES ? next.slice(-MAX_ENTRIES) : next;
  if (autoScroll.value) {
    nextTick(() => {
      scroller.value?.scrollTo({ top: scroller.value.scrollHeight });
    });
  }
}

async function start() {
  await stop();
  const current = generation;

  const connectionId = getSelectedConnectionId();
  if (!connectionId) {
    error.value = "Select or create a connection first.";
    return;
  }

  entries.value = [];
  error.value = "";
  streamStatus.value = "connecting";

  const onEvent = (event: LogEvent) => {
    if (current !== generation) return;
    if (event.kind === "lines") {
      if (streamStatus.value === "connecting") streamStatus.value = live.value ? "live" : "ended";
      append(event.lines);
    } else if (event.kind === "error") {
      streamStatus.value = "error";
      error.value = event.message;
    } else {
      streamStatus.value = live.value ? "error" : "ended";
      if (live.value) error.value = "The server closed the log stream. Refresh to reconnect.";
    }
  };

  try {
    const stopper = await streamLogs(
      connectionId,
      { numLines: numLines.value, follow: live.value },
      onEvent,
    );
    if (current !== generation) {
      await stopper();
      return;
    }
    stopStream = stopper;
    // A follow stream with no backlog sends nothing until the next log line.
    if (live.value && streamStatus.value === "connecting") streamStatus.value = "live";
  } catch (err) {
    streamStatus.value = "error";
    error.value = String(err);
  }
}

// Scrolling up to read older lines pauses auto-scroll; returning to the
// bottom resumes it.
function onScroll() {
  const el = scroller.value;
  if (!el) return;
  autoScroll.value = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
}

watch(live, start);
watch(numLines, (value, previous) => {
  if (value !== previous && value > 0) start();
});

onMounted(() => {
  start();
  setPageRefreshHandler(start);
});

onUnmounted(() => {
  stop();
  clearPageRefreshHandler(start);
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-3 p-4">
    <div class="shrink-0 flex flex-wrap items-center justify-between gap-2">
      <div class="flex flex-wrap items-center gap-2">
        <UInput
          v-model="search"
          icon="i-lucide-search"
          placeholder="Filter messages"
          aria-label="Filter log messages"
          class="w-56"
        />
        <USelect v-model="levelFilter" :items="LEVEL_FILTERS" class="w-36" aria-label="Minimum level" />
        <UBadge color="neutral" variant="subtle">
          {{ visibleEntries.length }}/{{ entries.length }} lines
        </UBadge>
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <UBadge
          :color="STATUS_BADGES[streamStatus].color"
          variant="subtle"
          :icon="streamStatus === 'live' ? 'i-lucide-radio' : undefined"
        >
          {{ STATUS_BADGES[streamStatus].label }}
        </UBadge>
        <USwitch v-model="live" label="Follow" />
        <UCheckbox v-model="autoScroll" label="Auto-scroll" />
        <UInput
          v-model.number="numLines"
          type="number"
          min="1"
          max="10000"
          class="w-24"
          aria-label="Backlog lines"
          title="Lines of history to load"
        />
        <UButton
          icon="i-lucide-eraser"
          color="neutral"
          variant="soft"
          aria-label="Clear view"
          title="Clear view"
          @click="entries = []"
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
      ref="scroller"
      class="min-h-0 flex-1 overflow-auto rounded-lg border border-default bg-default/30 font-mono text-xs"
      @scroll="onScroll"
    >
      <div
        v-for="log in visibleEntries"
        :key="log.id"
        class="grid grid-cols-[auto_auto_1fr] items-start gap-x-3 border-b border-default/40 px-3 py-1.5 hover:bg-elevated/40"
      >
        <UIcon
          :name="LEVEL_STYLES[log.level].icon"
          class="mt-0.5 size-3.5"
          :class="LEVEL_STYLES[log.level].text"
        />
        <span class="whitespace-nowrap text-muted">{{ log.timestamp }}</span>
        <div class="min-w-0">
          <span class="whitespace-pre-wrap break-words text-highlighted">{{ log.message }}</span>
          <span v-if="log.source" class="ml-2 text-dimmed">{{ log.source }}</span>
        </div>
      </div>
      <p v-if="!visibleEntries.length" class="p-3 text-sm text-muted">
        {{
          streamStatus === "connecting"
            ? "Connecting..."
            : entries.length
              ? "No lines match the filter."
              : "No logs yet."
        }}
      </p>
    </div>
  </div>
</template>
