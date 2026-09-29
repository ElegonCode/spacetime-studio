<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { NavigationMenuItem } from "@nuxt/ui";
import { useRoute, useRouter } from "vue-router";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import {
  availableUpdate,
  checkForUpdate,
} from "./lib/updateStore";
import {
  pageRefreshHandler,
  pageRefreshLoading,
  runPageRefresh,
} from "./lib/pageActions";
import { useConnections } from "./lib/connectionStore";
import { pageNavigationLoadingPath } from "./router";
import ConnectionPicker from "./components/ConnectionPicker.vue";

const open = ref(true);
const startupReady = ref(false);
const route = useRoute();
const router = useRouter();
const updateProgress = ref<number | null>(null);
const updateError = ref<string | null>(null);
const installingUpdate = ref(false);
let releaseCheckTimer: ReturnType<typeof setInterval> | undefined;
async function installUpdate() {
  const initialUpdate = availableUpdate.value;
  if (!initialUpdate || installingUpdate.value) return;
  const isRetry = Boolean(updateError.value);
  installingUpdate.value = true;
  updateError.value = null;
  updateProgress.value = null;
  let updateToInstall = initialUpdate;
  let downloaded = 0;
  let contentLength = 0;
  try {
    if (isRetry) {
      // A failed attempt may have used a manifest or asset URL that was still
      // propagating. Retry against the current release metadata instead.
      await updateToInstall.close();
      const refreshedUpdate = await check();
      if (!refreshedUpdate) {
        throw new Error("No update is currently available. Restart the app to check its installed version.");
      }
      updateToInstall = refreshedUpdate;
      availableUpdate.value = refreshedUpdate;
    }

    await updateToInstall.downloadAndInstall((event) => {
      if (event.event === "Started") {
        contentLength = event.data.contentLength ?? 0;
        updateProgress.value = 0;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        updateProgress.value = contentLength > 0
          ? Math.min(100, Math.round((downloaded / contentLength) * 100))
          : null;
      } else if (event.event === "Finished") {
        updateProgress.value = 100;
      }
    });
    await relaunch();
  } catch (error) {
    console.error("Failed to install application update", error);
    updateError.value = error instanceof Error ? error.message : String(error);
    installingUpdate.value = false;
    updateProgress.value = null;
  }
}

async function dismissUpdate() {
  await availableUpdate.value?.close();
  availableUpdate.value = null;
}

const { selectedId, selectedConnection, canAccess, loadConnections } = useConnections();

// Switching to a different connection should reload whatever page is open so it
// shows data for the newly active connection.
watch(selectedId, (newId, oldId) => {
  if (newId && oldId && newId !== oldId) {
    runPageRefresh("connection-change");
  }
});

// A connection that turns out to be unreachable locks database pages, while
// settings remains available so the connection can be fixed.
watch(canAccess, (allowed) => {
  if (!allowed && !["/tables", "/settings"].includes(route.path)) {
    router.push("/tables");
  }
});

onMounted(async () => {
  await router.isReady().catch(() => {});
  startupReady.value = true;
  loadConnections();
  void checkForUpdate();
  releaseCheckTimer = setInterval(() => void checkForUpdate(), 6 * 60 * 60 * 1000);
});

onUnmounted(() => {
  if (releaseCheckTimer) clearInterval(releaseCheckTimer);
  void availableUpdate.value?.close();
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
  "/settings": {
    title: "Settings",
    description: "Manage application and connection settings.",
  },
};

const pageHeader = computed(
  () =>
    pageHeaders[route.path] ?? {
      title: "Spacetime Studio",
      description: "SpacetimeDB HTTP API admin workspace.",
    },
);

const baseItems: NavigationMenuItem[] = [
  {
    label: "Tables",
    icon: "i-lucide-table",
    to: "/tables",
    slot: "page",
  },
  {
    label: "SQL",
    icon: "i-lucide-square-terminal",
    to: "/sql",
    slot: "page",
  },
  {
    label: "Functions",
    icon: "i-lucide-square-function",
    to: "/functions",
    slot: "page",
  },
  {
    label: "Logs",
    icon: "i-lucide-clipboard-clock",
    to: "/logs",
    slot: "page",
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

function isPageLoading(item: NavigationMenuItem) {
  return pageNavigationLoadingPath.value === item.to;
}
</script>

<template>
  <UApp :toaster="{ position: 'bottom-right', progress: true, duration: 4000 }">
    <div v-if="!startupReady" class="launch-screen" role="status" aria-live="polite">
      <div class="launch-mark" aria-hidden="true">
        <span class="launch-spinner" />
        <span class="launch-core" />
      </div>
      <p class="launch-title">Spacetime Studio</p>
      <p class="launch-caption">Loading your workspace</p>
    </div>
    <div v-else class="flex h-screen min-h-0 bg-default">
      <USidebar
        v-model:open="open"
        title="Spacetime Studio"
        collapsible="icon"
        class="select-none"
        :ui="{ footer: 'p-1' }"
      >
        <template #default="{ state }">
          <ConnectionPicker :collapsed="state === 'collapsed'" />

          <UNavigationMenu
            :items="items"
            orientation="vertical"
            :ui="{ link: 'p-1.5 overflow-hidden' }"
          >
            <template #page-trailing="{ item }">
              <UIcon
                v-if="isPageLoading(item)"
                name="i-lucide-loader-circle"
                class="size-4 animate-spin text-muted"
                aria-hidden="true"
              />
            </template>
          </UNavigationMenu>
        </template>

        <template #footer="{ state }">
          <div class="flex w-full flex-col gap-1">
            <UButton
              :icon="'i-lucide-settings'"
              :color="route.path === '/settings' ? 'primary' : 'neutral'"
              :variant="route.path === '/settings' ? 'soft' : 'ghost'"
              :class="state === 'collapsed' ? 'mx-auto' : 'w-full justify-start'"
              :aria-label="state === 'collapsed' ? 'Settings' : undefined"
              title="Settings"
              @click="router.push('/settings')"
            >
              <span v-if="state !== 'collapsed'">Settings</span>
              <template #trailing>
                <UIcon
                  v-if="pageNavigationLoadingPath === '/settings'"
                  name="i-lucide-loader-circle"
                  class="size-4 animate-spin"
                  aria-hidden="true"
                />
              </template>
            </UButton>
            <div v-if="availableUpdate">
              <UAlert
                v-if="state !== 'collapsed'"
                color="primary"
                variant="subtle"
                icon="i-lucide-download"
                :title="updateError ? 'Update failed' : `Update ${availableUpdate.version} available`"
                :description="updateError ? `The update could not be installed: ${updateError}` : updateProgress !== null ? `Downloading update: ${updateProgress}%` : 'A new version is ready to install.'"
                :ui="{ root: 'items-start' }"
              >
                <template #actions>
                  <UButton
                    size="xs"
                    color="primary"
                    variant="soft"
                    icon="i-lucide-download"
                    :loading="installingUpdate"
                    :disabled="installingUpdate"
                    @click="installUpdate"
                  >
                    {{ updateError ? 'Retry update' : updateProgress !== null ? 'Installing' : 'Update now' }}
                  </UButton>
                  <UButton size="xs" color="neutral" variant="ghost" :disabled="installingUpdate" @click="dismissUpdate">Later</UButton>
                </template>
              </UAlert>
              <UButton
                v-else
                class="mx-auto flex"
                color="primary"
                variant="soft"
                icon="i-lucide-download"
                aria-label="New update available"
                :title="`Update ${availableUpdate.version} available`"
                @click="open = true"
              />
            </div>
          </div>
        </template>
      </USidebar>

      <div class="flex-1 min-w-0 flex flex-col">
        <div
          class="h-(--ui-header-height) shrink-0 flex items-center gap-3 px-4 border-b border-default"
        >
          <UButton
            size="xs"
            :icon="open ? 'i-lucide-panel-left-close' : 'i-lucide-panel-left-open'"
            color="neutral"
            variant="ghost"
            :aria-label="open ? 'Collapse sidebar' : 'Expand sidebar'"
            :title="open ? 'Collapse sidebar' : 'Expand sidebar'"
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
          <div id="page-header-actions" class="flex min-w-0 items-center gap-2" />
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
            @click="() => runPageRefresh()"
          />
        </div>

        <main class="min-h-0 flex-1 overflow-hidden">
          <RouterView />
        </main>
      </div>
    </div>
  </UApp>
</template>
