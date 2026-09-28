<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { useConnections } from "../lib/connectionStore";
import { openConnectionEditor } from "../lib/connectionUi";
import { themePreference } from "../lib/theme";
import {
  availableUpdate,
  checkForUpdate,
  checkingForUpdate,
  lastUpdateCheck,
  updateCheckCooldown,
  updateCheckFailed,
} from "../lib/updateStore";

const { selectedConnection, status, statusMessage } = useConnections();
const appVersion = ref("Loading…");
const formattedLastUpdateCheck = computed(() =>
  lastUpdateCheck.value
    ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(lastUpdateCheck.value)
    : "Never",
);

const connectionStatus = computed(() => {
  if (!selectedConnection.value) return "No connection selected";
  if (status.value === "connected") return "Connected";
  if (status.value === "checking") return "Checking connection";
  if (status.value === "error") return statusMessage.value || "Connection unavailable";
  return "Not checked";
});

onMounted(async () => {
  try {
    appVersion.value = await getVersion();
  } catch {
    appVersion.value = "Unavailable";
  }
});
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-3xl space-y-6">
      <section>
        <h1 class="text-lg font-semibold text-highlighted">Settings</h1>
        <p class="mt-1 text-sm text-muted">Configure Spacetime Studio and its database connections.</p>
      </section>

      <UCard>
        <div class="flex items-start justify-between gap-4">
          <div>
            <h2 class="font-medium text-highlighted">Active connection</h2>
            <p class="mt-1 text-sm text-muted">Manage the database profile used by the workspace.</p>
          </div>
          <UButton
            icon="i-lucide-pencil"
            color="neutral"
            variant="soft"
            :disabled="!selectedConnection"
            @click="openConnectionEditor(selectedConnection)"
          >
            Edit connection
          </UButton>
        </div>

        <dl class="mt-5 grid gap-4 border-t border-default pt-4 sm:grid-cols-2">
          <div>
            <dt class="text-xs font-medium uppercase tracking-wide text-muted">Name</dt>
            <dd class="mt-1 text-sm text-highlighted">{{ selectedConnection?.name ?? 'None' }}</dd>
          </div>
          <div>
            <dt class="text-xs font-medium uppercase tracking-wide text-muted">Status</dt>
            <dd class="mt-1 text-sm text-highlighted">{{ connectionStatus }}</dd>
          </div>
          <div>
            <dt class="text-xs font-medium uppercase tracking-wide text-muted">Database</dt>
            <dd class="mt-1 break-all text-sm text-highlighted">{{ selectedConnection?.database ?? '—' }}</dd>
          </div>
          <div>
            <dt class="text-xs font-medium uppercase tracking-wide text-muted">Host</dt>
            <dd class="mt-1 break-all text-sm text-highlighted">{{ selectedConnection?.baseUrl ?? '—' }}</dd>
          </div>
        </dl>
      </UCard>

      <UCard>
        <div class="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h2 class="font-medium text-highlighted">Appearance</h2>
            <p class="mt-1 text-sm text-muted">Choose how Spacetime Studio looks.</p>
          </div>
          <label class="flex items-center gap-3 text-sm text-highlighted">
            <span>Theme</span>
            <select
              v-model="themePreference"
              class="rounded-md border border-default bg-default px-3 py-2 text-sm text-highlighted focus-visible:outline-2 focus-visible:outline-primary"
              aria-label="Theme"
            >
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
        </div>
      </UCard>

      <UCard>
        <h2 class="font-medium text-highlighted">About</h2>
        <dl class="mt-4 border-t border-default pt-4">
          <dt class="text-xs font-medium uppercase tracking-wide text-muted">Spacetime Studio</dt>
          <dd class="mt-1 text-sm text-highlighted">Version {{ appVersion }}</dd>
        </dl>
      </UCard>

      <UCard>
        <div class="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h2 class="font-medium text-highlighted">Updates</h2>
            <p class="mt-1 text-sm text-muted">Last checked: {{ formattedLastUpdateCheck }}</p>
            <p v-if="updateCheckFailed" class="mt-1 text-sm text-error">
              Could not check for updates. Check your connection and try again later.
            </p>
            <p v-else-if="availableUpdate" class="mt-1 text-sm text-primary">
              Version {{ availableUpdate.version }} is available.
            </p>
            <p v-else-if="lastUpdateCheck" class="mt-1 text-sm text-muted">
              No update was found.
            </p>
          </div>
          <UButton
            icon="i-lucide-refresh-cw"
            color="neutral"
            variant="soft"
            :loading="checkingForUpdate"
            :disabled="checkingForUpdate || updateCheckCooldown > 0"
            @click="checkForUpdate"
          >
            {{ updateCheckCooldown > 0 ? `Check again in ${updateCheckCooldown}s` : 'Check for updates' }}
          </UButton>
        </div>
      </UCard>
    </div>
  </div>
</template>
