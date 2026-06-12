<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  getSchema,
  getSelectedConnectionId,
  type FunctionSummary,
  type SchemaSummary,
} from "../lib/spacetime";
import { clearPageRefreshHandler, setPageRefreshHandler } from "../lib/pageActions";

const schema = ref<SchemaSummary | null>(null);
const loading = ref(false);
const error = ref("");
const filter = ref("");

const functions = computed<FunctionSummary[]>(() => {
  const query = filter.value.toLowerCase();
  return (schema.value?.reducers ?? []).filter((item) => item.name.toLowerCase().includes(query));
});

async function load() {
  const connectionId = getSelectedConnectionId();
  if (!connectionId) {
    error.value = "Select or create a connection first.";
    return;
  }

  loading.value = true;
  error.value = "";

  try {
    schema.value = await getSchema(connectionId);
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
  <div class="space-y-4">
    <UInput v-model="filter" icon="i-lucide-search" placeholder="Filter functions" class="max-w-md" />
    <UAlert v-if="error" color="error" variant="subtle" :description="error" />

    <div class="grid gap-3 lg:grid-cols-2">
      <div
        v-for="fn in functions"
        :key="fn.name"
        class="rounded-lg border border-default bg-default/30 p-4"
      >
        <div class="flex items-center justify-between gap-3">
          <h2 class="truncate text-base font-semibold text-highlighted">{{ fn.name }}</h2>
          <UBadge color="neutral" variant="subtle">{{ fn.lifecycle ?? "callable" }}</UBadge>
        </div>
        <div class="mt-3 flex flex-wrap gap-2">
          <UBadge v-for="param in fn.params" :key="param.name" variant="soft" color="neutral">
            {{ param.name }}: {{ param.type }}
          </UBadge>
          <span v-if="fn.params.length === 0" class="text-sm text-muted">No parameters</span>
        </div>
      </div>
    </div>
  </div>
</template>
