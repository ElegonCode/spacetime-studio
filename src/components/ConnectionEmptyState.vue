<script setup lang="ts">
import { computed } from "vue";
import { useConnections } from "../lib/connectionStore";
import { openConnectionEditor } from "../lib/connectionUi";

// Shown in place of page content while there is no usable connection. Managing
// connections lives in the sidebar picker.
const {
  connections,
  selectedConnection,
  status,
  statusMessage,
  lastCheckFailed,
  loading,
  checkConnection,
} = useConnections();

const failed = computed(() => lastCheckFailed.value && selectedConnection.value !== null);
</script>

<template>
  <div class="flex h-full items-center justify-center p-6">
    <div v-if="loading && connections.length === 0" class="text-sm text-muted">
      Loading connections...
    </div>

    <div v-else class="max-w-sm text-center">
      <div
        class="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-elevated"
      >
        <UIcon
          :name="failed ? 'i-lucide-unplug' : 'i-lucide-plug'"
          class="size-6"
          :class="failed ? 'text-red-400' : 'text-muted'"
        />
      </div>

      <template v-if="failed">
        <h2 class="text-base font-semibold text-highlighted">
          Can't reach {{ selectedConnection?.name }}
        </h2>
        <p class="mt-1 text-sm text-muted">
          Pick another connection from the sidebar, or edit this one.
        </p>
        <p
          v-if="statusMessage"
          class="mt-3 rounded-md bg-elevated px-3 py-2 text-left font-mono text-xs break-words text-muted"
        >
          {{ statusMessage }}
        </p>
        <div class="mt-4 flex justify-center gap-2">
          <UButton
            icon="i-lucide-refresh-cw"
            color="neutral"
            variant="soft"
            :loading="status === 'checking'"
            @click="checkConnection"
          >
            Retry
          </UButton>
          <UButton
            icon="i-lucide-pencil"
            color="neutral"
            variant="soft"
            @click="openConnectionEditor(selectedConnection)"
          >
            Edit connection
          </UButton>
        </div>
      </template>

      <template v-else>
        <h2 class="text-base font-semibold text-highlighted">Connect to a database</h2>
        <p class="mt-1 text-sm text-muted">
          Add a local, Maincloud, or self-hosted SpacetimeDB database. You can switch between
          connections from the sidebar at any time.
        </p>
        <UButton icon="i-lucide-plus" class="mt-4" @click="openConnectionEditor(null)">
          Add connection
        </UButton>
      </template>
    </div>
  </div>
</template>
