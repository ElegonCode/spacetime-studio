<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { NavigationMenuItem } from "@nuxt/ui";
import { defineShortcuts } from "@nuxt/ui/composables";
import { useRoute, useRouter } from "vue-router";
import { openUrl } from "@tauri-apps/plugin-opener";
import packageJson from "../package.json";
import {
  pageRefreshHandler,
  pageRefreshLoading,
  runPageRefresh,
} from "./lib/pageActions";
import { useConnections } from "./lib/connectionStore";
import ConnectionPicker from "./components/ConnectionPicker.vue";

const open = ref(true);
const route = useRoute();
const router = useRouter();
const availableRelease = ref<{ version: string; url: string } | null>(null);
let releaseCheckTimer: ReturnType<typeof setInterval> | undefined;

function compareVersions(left: string, right: string): number {
  const parse = (version: string) => {
    const match = version.replace(/^v/i, "").match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?/);
    if (!match) return null;
    return {
      core: match.slice(1, 4).map(Number),
      prerelease: match[4]?.split(".") ?? null,
    };
  };
  const a = parse(left);
  const b = parse(right);
  if (!a || !b) return 0;
  for (let index = 0; index < 3; index += 1) {
    if (a.core[index] !== b.core[index]) return a.core[index] - b.core[index];
  }
  if (!a.prerelease && b.prerelease) return 1;
  if (a.prerelease && !b.prerelease) return -1;
  if (!a.prerelease || !b.prerelease) return 0;
  for (let index = 0; index < Math.max(a.prerelease.length, b.prerelease.length); index += 1) {
    const aPart = a.prerelease[index];
    const bPart = b.prerelease[index];
    if (aPart === undefined) return -1;
    if (bPart === undefined) return 1;
    if (aPart === bPart) continue;
    const aNumber = /^\d+$/.test(aPart) ? Number(aPart) : null;
    const bNumber = /^\d+$/.test(bPart) ? Number(bPart) : null;
    if (aNumber !== null && bNumber !== null) return aNumber - bNumber;
    if (aNumber !== null) return -1;
    if (bNumber !== null) return 1;
    return aPart.localeCompare(bPart);
  }
  return 0;
}

async function checkForRelease() {
  try {
    const response = await fetch(
      "https://api.github.com/repos/ElegonCode/spacetime-studio/releases/latest",
      { headers: { Accept: "application/vnd.github+json" } },
    );
    if (!response.ok) return;
    const release = (await response.json()) as { tag_name?: string; html_url?: string };
    if (
      release.tag_name &&
      release.html_url &&
      compareVersions(release.tag_name, packageJson.version) > 0
    ) {
      availableRelease.value = { version: release.tag_name.replace(/^v/i, ""), url: release.html_url };
    } else {
      availableRelease.value = null;
    }
  } catch {
    // A failed or offline check should not interrupt the workspace.
  }
}

const { selectedId, selectedConnection, canAccess, loadConnections } = useConnections();

// Switching to a different connection should reload whatever page is open so it
// shows data for the newly active connection.
watch(selectedId, (newId, oldId) => {
  if (newId && oldId && newId !== oldId) {
    runPageRefresh();
  }
});

// A connection that turns out to be unreachable locks every page but tables, so
// send the user back there even without an explicit navigation.
watch(canAccess, (allowed) => {
  if (!allowed && route.path !== "/tables") {
    router.push("/tables");
  }
});

onMounted(() => {
  loadConnections();
  void checkForRelease();
  releaseCheckTimer = setInterval(() => void checkForRelease(), 6 * 60 * 60 * 1000);
});

onUnmounted(() => {
  if (releaseCheckTimer) clearInterval(releaseCheckTimer);
});

const pageHeaders: Record<string, { title: string; description: string }> = {
  "/tables": {
    title: "Tables",
    description:
      "Browse schema, page through rows, and edit table data.",
  },
  "/sql": {
    title: "SQL",
    description: "Run SQL against the database and export the results.",
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
    label: "Tables",
    icon: "i-lucide-table",
    to: "/tables",
  },
  {
    label: "SQL",
    icon: "i-lucide-square-terminal",
    to: "/sql",
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

// Every page but tables needs a reachable connection, so those links stay
// disabled until one is confirmed usable.
const items = computed<NavigationMenuItem[]>(() =>
  baseItems.map((item) => ({
    ...item,
    disabled: item.to !== "/tables" && !canAccess.value,
  })),
);
</script>

<template>
  <UApp :toaster="{ position: 'bottom-right', progress: true, duration: 4000 }">
    <div class="flex h-screen min-h-0 bg-neutral-950">
      <USidebar v-model:open="open" title="Spacetime Studio" collapsible="icon" class="select-none">
        <template #default="{ state }">
          <ConnectionPicker :collapsed="state === 'collapsed'" />

          <UNavigationMenu
            :items="items"
            orientation="vertical"
            :ui="{ link: 'p-1.5 overflow-hidden' }"
          />
        </template>

        <template #footer="{ state }">
          <div v-if="availableRelease" class="w-full p-2">
            <UAlert
              v-if="state !== 'collapsed'"
              color="success"
              variant="subtle"
              icon="i-lucide-download"
              :title="`Update ${availableRelease.version} available`"
              description="A new version of Spacetime Studio is ready to download."
              :ui="{ root: 'items-start' }"
            >
              <template #actions>
                <UButton
                  size="xs"
                  color="success"
                  variant="soft"
                  icon="i-lucide-external-link"
                  @click="openUrl(availableRelease!.url)"
                >
                  View release
                </UButton>
              </template>
            </UAlert>
            <UButton
              v-else
              class="mx-auto flex"
              color="success"
              variant="soft"
              icon="i-lucide-download"
              aria-label="New update available"
              :title="`Update ${availableRelease.version} available`"
              @click="openUrl(availableRelease!.url)"
            />
          </div>
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
            @click="() => { open = !open }"
          />
          <div class="min-w-0 flex-1 select-none">
            <p class="truncate text-sm font-medium text-highlighted">
              {{ pageHeader.title }}
            </p>
            <p class="truncate text-xs text-muted">
              {{ pageHeader.description }}
            </p>
          </div>
          <UBadge
            v-if="selectedConnection?.readOnly"
            color="neutral"
            variant="subtle"
            icon="i-lucide-lock"
            title="Writes are blocked for this connection. Change it in the connection settings."
          >
            Read-only
          </UBadge>
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
