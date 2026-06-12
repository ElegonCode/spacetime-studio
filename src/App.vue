<script setup lang="ts">
import { computed, ref } from "vue";
import type { NavigationMenuItem } from "@nuxt/ui";
import { useRoute } from "vue-router";
import {
  pageRefreshHandler,
  pageRefreshLoading,
  runPageRefresh,
} from "./lib/pageActions";

const open = ref(true);
const route = useRoute();

const pageHeaders: Record<string, { title: string; description: string }> = {
  "/": {
    title: "Connections",
    description:
      "Manage local and hosted SpacetimeDB profiles without exposing tokens to the frontend.",
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

const items: NavigationMenuItem[] = [
  {
    label: "Connections",
    icon: "i-lucide-plug",
    to: "/",
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
</script>

<template>
  <UApp>
    <div class="flex h-screen min-h-0 bg-neutral-950">
      <USidebar v-model:open="open" title="Spacetime Studio" collapsible="icon">
        <UNavigationMenu
          :items="items"
          orientation="vertical"
          :ui="{ link: 'p-1.5 overflow-hidden' }"
        />
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
          <div class="min-w-0 flex-1">
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

        <main class="min-h-0 flex-1 overflow-hidden p-4">
          <RouterView />
        </main>
      </div>
    </div>
  </UApp>
</template>
