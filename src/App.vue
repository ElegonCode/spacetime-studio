<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import type { NavigationMenuItem, SelectItem } from "@nuxt/ui";
import { useRoute, useRouter } from "vue-router";
import {
  pageRefreshHandler,
  pageRefreshLoading,
  runPageRefresh,
} from "./lib/pageActions";
import { useConnections } from "./lib/connectionStore";

const open = ref(true);
const route = useRoute();
const router = useRouter();

const {
  connections,
  selectedId,
  selectedConnection,
  status,
  statusMessage,
  canAccess,
  loadConnections,
  selectConnection,
} = useConnections();

const connectionItems = computed<SelectItem[]>(() =>
  connections.value.map((connection) => ({
    label: connection.name,
    value: connection.id,
  })),
);

// The leading indicator on the switcher: green only once a test has actually
// succeeded, amber while checking, red on a failed check, grey when idle.
const statusDotClass = computed(() => {
  switch (status.value) {
    case "connected":
      return "bg-green-500";
    case "checking":
      return "bg-amber-500 animate-pulse";
    case "error":
      return "bg-red-500";
    default:
      return "bg-neutral-500";
  }
});

const statusLabel = computed(() => {
  switch (status.value) {
    case "connected":
      return "Connected";
    case "checking":
      return "Checking connection...";
    case "error":
      return statusMessage.value ? `Not connected: ${statusMessage.value}` : "Not connected";
    default:
      return "No connection";
  }
});

function onSelectConnection(id: unknown) {
  if (typeof id !== "string" || id === selectedId.value) return;
  selectConnection(id);
}

// Switching to a different connection should reload whatever page is open so it
// shows data for the newly active connection.
watch(selectedId, (newId, oldId) => {
  if (newId && oldId && newId !== oldId) {
    runPageRefresh();
  }
});

// A connection that turns out to be unreachable locks the workspace, so send the
// user to the connections page even without an explicit navigation.
watch(canAccess, (allowed) => {
  if (!allowed && route.path !== "/") {
    router.push("/");
  }
});

onMounted(() => {
  loadConnections();
});

const pageHeaders: Record<string, { title: string; description: string }> = {
  "/": {
    title: "Connections",
    description:
      "Manage local and hosted SpacetimeDB profiles without exposing tokens to the frontend.",
  },
  "/overview": {
    title: "Overview",
    description:
      "Live client connections and how much space each table is using.",
  },
  "/tables": {
    title: "Tables",
    description:
      "Browse schema, page through rows, and run generic SQL mutations.",
  },
  "/functions": {
    title: "Functions",
    description:
      "Reducers, procedures, and lifecycle functions from the schema.",
  },
  "/logs": {
    title: "Logs",
    description:
      "Owner/admin credentials are required for private database logs.",
  },
};

const pageHeader = computed(
  () =>
    pageHeaders[route.path] ?? {
      title: "Spacetime Studio",
      description: "SpacetimeDB HTTP API admin workspace.",
    },
);

defineShortcuts({
  o: () => (open.value = !open.value),
});

const baseItems: NavigationMenuItem[] = [
  {
    label: "Connections",
    icon: "i-lucide-plug",
    to: "/",
  },
  {
    label: "Overview",
    icon: "i-lucide-layout-dashboard",
    to: "/overview",
  },
  {
    label: "Tables",
    icon: "i-lucide-table",
    to: "/tables",
  },
  {
    label: "Functions",
    icon: "i-lucide-square-function",
    to: "/functions",
  },
  {
    label: "Logs",
    icon: "i-lucide-clipboard-clock",
    to: "/logs",
  },
];

// Everything except the connections page needs a reachable connection, so those
// links stay disabled until one is confirmed usable.
const items = computed<NavigationMenuItem[]>(() =>
  baseItems.map((item) => ({
    ...item,
    disabled: item.to !== "/" && !canAccess.value,
  })),
);
</script>

<template>
  <UApp>
    <div class="flex h-screen min-h-0 bg-neutral-950">
      <USidebar v-model:open="open" title="Spacetime Studio" collapsible="icon" class="select-none">
        <template #default="{ state }">
          <div class="overflow-hidden">
            <USelect
              v-if="state !== 'collapsed'"
              :model-value="selectedId ?? undefined"
              :items="connectionItems"
              value-key="value"
              placeholder="No connection"
              :disabled="connections.length === 0"
              class="w-full"
              :ui="{ base: 'w-full' }"
              @update:model-value="onSelectConnection"
            >
              <template #leading>
                <span
                  class="size-2 shrink-0 rounded-full"
                  :class="statusDotClass"
                  :aria-label="statusLabel"
                  :title="statusLabel"
                />
              </template>
              <template #default>
                <span class="truncate">
                  {{ selectedConnection?.name ?? "No connection" }}
                </span>
              </template>
            </USelect>
            <div v-else class="flex justify-center py-1.5" :title="statusLabel">
              <span class="size-2.5 rounded-full" :class="statusDotClass" />
            </div>
          </div>

          <UNavigationMenu
            :items="items"
            orientation="vertical"
            :ui="{ link: 'p-1.5 overflow-hidden' }"
          />
        </template>
      </USidebar>

      <div class="flex-1 min-w-0 flex flex-col">
        <div
          class="h-(--ui-header-height) shrink-0 flex items-center gap-3 px-4 border-b border-default"
        >
          <UButton
            icon="i-lucide-panel-left"
            color="neutral"
            variant="ghost"
            :aria-label="open ? 'Close sidebar' : 'Open sidebar'"
            @click="open = !open"
          />
          <div class="min-w-0 flex-1 select-none">
            <p class="truncate text-sm font-medium text-highlighted">
              {{ pageHeader.title }}
            </p>
            <p class="truncate text-xs text-muted">
              {{ pageHeader.description }}
            </p>
          </div>
          <UButton
            icon="i-lucide-refresh-cw"
            color="neutral"
            variant="soft"
            :loading="pageRefreshLoading"
            :disabled="!pageRefreshHandler"
            aria-label="Refresh page"
            @click="runPageRefresh"
          />
        </div>

        <main class="min-h-0 flex-1 overflow-hidden">
          <RouterView />
        </main>
      </div>
    </div>
  </UApp>
</template>
