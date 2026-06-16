<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from "vue";
import { getLogs, getSelectedConnectionId } from "../lib/spacetime";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";

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

    <pre
      class="min-h-0 flex-1 overflow-auto rounded-lg border border-default bg-black p-4 text-xs leading-5 text-neutral-100"
      >{{ logs || "No logs loaded." }}</pre
    >
  </div>
</template>
