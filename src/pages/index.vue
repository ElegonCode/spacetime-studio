<script setup lang="ts">
import { ref } from "vue";
import ConnectionPicker from "../components/ConnectionPicker.vue";
import type { ConnectionProfile } from "../lib/spacetime";

const activeConnection = ref<ConnectionProfile | null>(null);
</script>

<template>
  <div class="space-y-4">
    <ConnectionPicker @changed="activeConnection = $event" />

    <section class="rounded-lg border border-default bg-default/30 p-4">
      <div class="grid gap-4 md:grid-cols-4">
        <div>
          <p class="text-xs uppercase text-muted">Active</p>
          <p class="mt-1 truncate text-sm font-medium text-highlighted">
            {{ activeConnection?.name ?? "No connection selected" }}
          </p>
        </div>
        <div>
          <p class="text-xs uppercase text-muted">Host</p>
          <p class="mt-1 truncate text-sm text-highlighted">{{ activeConnection?.baseUrl ?? "-" }}</p>
        </div>
        <div>
          <p class="text-xs uppercase text-muted">Database</p>
          <p class="mt-1 truncate text-sm text-highlighted">{{ activeConnection?.database ?? "-" }}</p>
        </div>
        <div>
          <p class="text-xs uppercase text-muted">Credential</p>
          <p class="mt-1 text-sm text-highlighted">
            {{ activeConnection?.hasToken ? "Stored in OS keychain" : "Anonymous/public" }}
          </p>
        </div>
      </div>
    </section>
  </div>
</template>
