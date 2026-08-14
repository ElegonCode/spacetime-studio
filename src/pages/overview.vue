<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  getOverview,
  getSelectedConnectionId,
  type DatabaseOverview,
} from "../lib/spacetime";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";

// A donut stays readable at a glance only while the slices stay few, so the
// long tail is folded into a single "Other" slice.
const CHART_SLICES = 5;
const SERIES_COLORS = [
  "var(--st-series-1)",
  "var(--st-series-2)",
  "var(--st-series-3)",
  "var(--st-series-4)",
  "var(--st-series-5)",
];
const OTHER_COLOR = "var(--st-series-6)";

const MEGABYTE = 1024 * 1024;
const RADIUS = 86;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
// Segments are separated by a gap in the surface, never by a stroke around them.
const SEGMENT_GAP = 3;

const connectionId = ref<string | null>(getSelectedConnectionId());
const overview = ref<DatabaseOverview | null>(null);
const loading = ref(false);
const error = ref("");
const activeTable = ref<string | null>(null);

const tables = computed(() =>
  (overview.value?.tables ?? []).map((table) => ({
    name: table.name,
    rowCount: table.rowCount ?? null,
    rowBytes: table.rowBytes,
    indexBytes: table.indexBytes,
    bytes: table.rowBytes + table.indexBytes,
  })),
);

const totalBytes = computed(() =>
  tables.value.reduce((total, table) => total + table.bytes, 0),
);

const sizedTables = computed(() => tables.value.filter((table) => table.bytes > 0));

const segments = computed(() => {
  const total = totalBytes.value;
  if (!total) return [];

  const head = sizedTables.value.slice(0, CHART_SLICES);
  const tail = sizedTables.value.slice(CHART_SLICES);

  const slices = head.map((table, index) => ({
    key: table.name,
    label: table.name,
    color: SERIES_COLORS[index],
    bytes: table.bytes,
    tables: [table.name],
  }));

  if (tail.length) {
    slices.push({
      key: "__other__",
      label: `Other (${tail.length} ${tail.length === 1 ? "table" : "tables"})`,
      color: OTHER_COLOR,
      bytes: tail.reduce((sum, table) => sum + table.bytes, 0),
      tables: tail.map((table) => table.name),
    });
  }

  let offset = 0;

  return slices.map((slice) => {
    const share = slice.bytes / total;
    const length = share * CIRCUMFERENCE;
    const gap = slices.length > 1 ? SEGMENT_GAP : 0;
    const segment = {
      ...slice,
      share,
      dash: Math.max(length - gap, 0.5),
      offset,
    };
    offset += length;
    return segment;
  });
});

const activeSegment = computed(
  () => segments.value.find((segment) => segment.key === activeTable.value) ?? null,
);

const connectionsSourceLabel = computed(() => {
  switch (overview.value?.connectionsSource) {
    case "st_client":
      return "Live from the st_client system table";
    case "metrics":
      return "Live from the host's connected-client gauge";
    default:
      return "Unavailable";
  }
});

const sizesSourceLabel = computed(() => {
  switch (overview.value?.sizesSource) {
    case "metrics":
      return "Measured from host metrics";
    case "estimate":
      return "Estimated from row counts and column types";
    default:
      return "Unavailable";
  }
});

function formatMegabytes(bytes: number) {
  const megabytes = bytes / MEGABYTE;
  if (megabytes === 0) return "0 MB";
  if (megabytes < 0.01) return "<0.01 MB";
  return `${megabytes.toFixed(megabytes < 10 ? 2 : 1)} MB`;
}

function formatShare(share: number) {
  if (share > 0 && share < 0.001) return "<0.1%";
  return `${(share * 100).toFixed(1)}%`;
}

function formatCount(value: number | null) {
  return value === null ? "-" : value.toLocaleString();
}

function shareOf(bytes: number) {
  return totalBytes.value ? bytes / totalBytes.value : 0;
}

function isHighlighted(tableName: string) {
  const segment = activeSegment.value;
  return segment !== null && segment.tables.includes(tableName);
}

async function load() {
  connectionId.value = getSelectedConnectionId();
  if (!connectionId.value) {
    error.value = "Select or create a connection first.";
    overview.value = null;
    return;
  }

  loading.value = true;
  error.value = "";

  try {
    overview.value = await getOverview(connectionId.value);
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  load();
  setPageRefreshHandler(load);
});

onUnmounted(() => {
  clearPageRefreshHandler(load);
});
</script>

<template>
  <div class="st-viz h-full min-h-0 overflow-y-auto">
    <div class="flex flex-col gap-4 p-4">
      <UAlert v-if="error" color="error" variant="subtle" :description="error" />

      <div class="grid gap-4 sm:grid-cols-3">
        <section class="rounded-lg border border-default bg-default/30 p-4">
          <p class="text-xs font-medium text-muted">Current connections</p>
          <p class="mt-2 text-5xl font-semibold leading-none text-highlighted">
            {{ overview?.connectedClients ?? (loading ? "..." : "-") }}
          </p>
          <p class="mt-3 text-xs text-muted">{{ connectionsSourceLabel }}</p>
          <p v-if="overview?.connectionsNote" class="mt-1 text-xs text-toned">
            {{ overview.connectionsNote }}
          </p>
        </section>

        <section class="rounded-lg border border-default bg-default/30 p-4">
          <p class="text-xs font-medium text-muted">Tables</p>
          <p class="mt-2 text-3xl font-semibold leading-none text-highlighted">
            {{ tables.length }}
          </p>
          <p class="mt-3 text-xs text-muted">
            {{ sizedTables.length }} holding data
          </p>
        </section>

        <section class="rounded-lg border border-default bg-default/30 p-4">
          <p class="text-xs font-medium text-muted">Total table size</p>
          <p class="mt-2 text-3xl font-semibold leading-none text-highlighted">
            {{ formatMegabytes(totalBytes) }}
          </p>
          <p
            v-if="overview?.blobStoreBytes"
            class="mt-3 text-xs text-muted"
          >
            Plus {{ formatMegabytes(overview.blobStoreBytes) }} in the blob store
          </p>
          <p v-else class="mt-3 text-xs text-muted">Rows and index keys</p>
        </section>
      </div>

      <UAlert
        v-if="overview?.sizesNote"
        color="neutral"
        variant="subtle"
        icon="i-lucide-info"
        :description="overview.sizesNote"
      />

      <div class="grid min-w-0 gap-4 xl:grid-cols-[minmax(0,400px)_minmax(0,1fr)]">
        <section class="rounded-lg border border-default bg-default/30 p-4">
          <h2 class="text-sm font-semibold text-highlighted">Size by table</h2>
          <p class="mt-1 text-xs text-muted">{{ sizesSourceLabel }}</p>

          <p
            v-if="!loading && !segments.length"
            class="py-10 text-center text-sm text-muted"
          >
            {{ tables.length ? "Every table is empty." : "No tables in this database." }}
          </p>

          <template v-else-if="segments.length">
            <div class="relative mx-auto mt-4 w-full max-w-[240px]">
              <svg viewBox="0 0 240 240" class="w-full" role="img"
                :aria-label="`Share of total table size by table, total ${formatMegabytes(totalBytes)}`">
                <circle
                  v-for="segment in segments"
                  :key="segment.key"
                  cx="120"
                  cy="120"
                  :r="RADIUS"
                  fill="none"
                  :stroke="segment.color"
                  :stroke-width="activeTable === segment.key ? 32 : 26"
                  :stroke-dasharray="`${segment.dash} ${CIRCUMFERENCE - segment.dash}`"
                  :stroke-dashoffset="-segment.offset"
                  transform="rotate(-90 120 120)"
                  class="cursor-pointer transition-[stroke-width] duration-150"
                  tabindex="0"
                  @mouseenter="activeTable = segment.key"
                  @mouseleave="activeTable = null"
                  @focus="activeTable = segment.key"
                  @blur="activeTable = null"
                />
              </svg>

              <div
                class="pointer-events-none absolute inset-0 flex flex-col items-center justify-center px-12 text-center"
              >
                <p class="w-full truncate text-xs text-muted">
                  {{ activeSegment?.label ?? "Total" }}
                </p>
                <p class="mt-1 text-xl font-semibold text-highlighted">
                  {{ formatMegabytes(activeSegment?.bytes ?? totalBytes) }}
                </p>
                <p v-if="activeSegment" class="mt-0.5 text-xs text-muted">
                  {{ formatShare(activeSegment.share) }}
                </p>
              </div>
            </div>

            <ul class="mt-5 space-y-1">
              <li
                v-for="segment in segments"
                :key="segment.key"
                class="flex items-center gap-2 rounded px-2 py-1 text-sm transition-colors"
                :class="activeTable === segment.key ? 'bg-elevated/60' : ''"
                @mouseenter="activeTable = segment.key"
                @mouseleave="activeTable = null"
              >
                <span
                  class="size-2.5 shrink-0 rounded-full"
                  :style="{ backgroundColor: segment.color }"
                  aria-hidden="true"
                />
                <span class="min-w-0 flex-1 truncate text-highlighted">
                  {{ segment.label }}
                </span>
                <span class="shrink-0 tabular-nums text-muted">
                  {{ formatMegabytes(segment.bytes) }}
                </span>
                <span class="w-12 shrink-0 text-right tabular-nums text-muted">
                  {{ formatShare(segment.share) }}
                </span>
              </li>
            </ul>
          </template>
        </section>

        <section
          class="min-w-0 overflow-hidden rounded-lg border border-default bg-default/30"
        >
          <div class="border-b border-default px-4 py-3">
            <h2 class="text-sm font-semibold text-highlighted">All tables</h2>
            <p class="mt-1 text-xs text-muted">
              Every table and its exact size, including the ones folded into "Other".
            </p>
          </div>

          <div class="overflow-x-auto">
            <table class="min-w-full text-sm">
              <thead class="bg-default/60">
                <tr>
                  <th
                    class="whitespace-nowrap border-b border-r border-default px-3 py-2 text-left text-xs font-medium text-muted"
                  >
                    Table
                  </th>
                  <th
                    class="whitespace-nowrap border-b border-r border-default px-3 py-2 text-right text-xs font-medium text-muted"
                  >
                    Rows
                  </th>
                  <th
                    class="whitespace-nowrap border-b border-r border-default px-3 py-2 text-right text-xs font-medium text-muted"
                  >
                    Row data
                  </th>
                  <th
                    class="whitespace-nowrap border-b border-r border-default px-3 py-2 text-right text-xs font-medium text-muted"
                  >
                    Indexes
                  </th>
                  <th
                    class="whitespace-nowrap border-b border-r border-default px-3 py-2 text-right text-xs font-medium text-muted"
                  >
                    Total
                  </th>
                  <th
                    class="whitespace-nowrap border-b border-default px-3 py-2 text-right text-xs font-medium text-muted"
                  >
                    Share
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="!loading && !tables.length">
                  <td class="px-3 py-6 text-muted" colspan="6">
                    No tables in this database.
                  </td>
                </tr>
                <tr
                  v-for="table in tables"
                  :key="table.name"
                  class="border-b border-default/60 transition-colors"
                  :class="isHighlighted(table.name) ? 'bg-elevated/60' : ''"
                >
                  <td
                    class="border-r border-default/60 px-3 py-2 font-medium text-highlighted"
                  >
                    {{ table.name }}
                  </td>
                  <td
                    class="whitespace-nowrap border-r border-default/60 px-3 py-2 text-right tabular-nums text-muted"
                  >
                    {{ formatCount(table.rowCount) }}
                  </td>
                  <td
                    class="whitespace-nowrap border-r border-default/60 px-3 py-2 text-right tabular-nums text-muted"
                  >
                    {{ formatMegabytes(table.rowBytes) }}
                  </td>
                  <td
                    class="whitespace-nowrap border-r border-default/60 px-3 py-2 text-right tabular-nums text-muted"
                  >
                    {{ formatMegabytes(table.indexBytes) }}
                  </td>
                  <td
                    class="whitespace-nowrap border-r border-default/60 px-3 py-2 text-right tabular-nums text-highlighted"
                  >
                    {{ formatMegabytes(table.bytes) }}
                  </td>
                  <td
                    class="whitespace-nowrap px-3 py-2 text-right tabular-nums text-muted"
                  >
                    {{ formatShare(shareOf(table.bytes)) }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<!-- Not scoped: `:global()` inside a scoped block drops the descendant part of
     the selector, so the dark override would never reach the chart. -->
<style>
/* Categorical slots, stepped per mode and validated for colour-vision
   deficiency against each surface. Slot 6 carries the folded "Other" slice. */
.st-viz {
  --st-series-1: #2a78d6;
  --st-series-2: #eb6834;
  --st-series-3: #1baf7a;
  --st-series-4: #eda100;
  --st-series-5: #e87ba4;
  --st-series-6: #008300;
}

.dark .st-viz {
  --st-series-1: #3987e5;
  --st-series-2: #d95926;
  --st-series-3: #199e70;
  --st-series-4: #c98500;
  --st-series-5: #d55181;
  --st-series-6: #008300;
}
</style>
