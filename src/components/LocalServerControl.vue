<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useToast } from "@nuxt/ui/composables";
import {
  serverOptions,
  serverStatus,
  serverAction,
  serverError,
  serverOutputOpen,
  validPort,
  startServer,
  stopServer,
  restartServer,
} from "../lib/localServerStore";
import {
  localInstallations,
  refreshInstallations,
  versionOperation,
} from "../lib/installationStore";

// The one server Studio can run on this computer. Every local connection is a
// database on it, so it gets a single power switch rather than one per row.
const emit = defineEmits<{ close: [] }>();
const toast = useToast();

const installed = computed(() => Boolean(localInstallations.value?.cliPath));
const state = computed(() => serverStatus.value?.state ?? "stopped");
const owned = computed(() => state.value === "starting" || state.value === "running");
const portValid = computed(() => validPort(serverOptions.value.port));
const showOptions = ref(!portValid.value);

const canToggle = computed(() => {
  if (serverAction.value || state.value === "external") return false;
  return owned.value || (portValid.value && !versionOperation.value);
});

const toggleTitle = computed(() => {
  if (state.value === "external")
    return "Started outside Studio, so it can only be stopped from there";
  if (owned.value) return "Stop the local server";
  if (versionOperation.value) return "Wait for the installation to finish";
  return portValid.value
    ? `Start on port ${serverOptions.value.port}`
    : "Choose a valid port first";
});

const now = ref(Date.now());
const clock = setInterval(() => (now.value = Date.now()), 1000);
const uptime = computed(() => {
  const startedAt = serverStatus.value?.startedAt;
  if (!startedAt) return "";
  const seconds = Math.max(0, Math.floor((now.value - startedAt) / 1000));
  const h = Math.floor(seconds / 3600),
    m = Math.floor((seconds % 3600) / 60),
    s = seconds % 60;
  return h ? `${h}h ${m}m` : m ? `${m}m ${s}s` : `${s}s`;
});

const address = computed(() => serverStatus.value?.url?.replace(/^https?:\/\//, "") ?? "");

const detail = computed(() => {
  const status = serverStatus.value;
  const port = serverOptions.value.port;
  if (serverAction.value === "stop") return { text: "Stopping…", class: "text-muted" };
  if (serverAction.value) return { text: "Starting…", class: "text-amber-400" };
  switch (state.value) {
    case "running": {
      const extras = [
        status?.options?.inMemory && "in memory",
        status?.options?.allowRemote && "on network",
      ].filter(Boolean);
      return {
        text: [address.value, `up ${uptime.value}`, ...extras].join(" · "),
        class: "text-green-400",
      };
    }
    case "starting":
      return {
        text: `Starting on port ${status?.options?.port ?? port}…`,
        class: "text-amber-400",
      };
    case "external":
      return { text: `Running outside Studio · port ${port}`, class: "text-sky-400" };
  }
  if (status?.exitMessage) return { text: "Stopped unexpectedly", class: "text-orange-400" };
  return {
    text: `Off · port ${validPort(port) ? port : "—"}${serverOptions.value.inMemory ? " · in memory" : ""}`,
    class: "text-muted",
  };
});

const ring = computed(() => {
  if (serverAction.value || state.value === "starting")
    return "bg-amber-500/15 text-amber-400 ring-amber-500/40";
  if (state.value === "running")
    return "bg-green-500/15 text-green-400 ring-green-500/40 hover:bg-green-500/25";
  if (state.value === "external") return "bg-sky-500/10 text-sky-400 ring-sky-500/30";
  return "bg-elevated text-muted ring-default hover:text-highlighted hover:ring-accented";
});

function toggle() {
  if (!canToggle.value) return;
  if (owned.value) void stopServer();
  else {
    showOptions.value = false;
    void startServer();
  }
}

async function copyAddress() {
  if (!serverStatus.value?.url) return;
  try {
    await navigator.clipboard.writeText(serverStatus.value.url);
    toast.add({
      title: "Server address copied",
      description: serverStatus.value.url,
      icon: "i-lucide-clipboard-check",
    });
  } catch {
    toast.add({ title: "Could not copy the address", color: "error" });
  }
}

function showOutput() {
  emit("close");
  serverOutputOpen.value = true;
}

onMounted(() => {
  if (!localInstallations.value) void refreshInstallations();
});
onUnmounted(() => clearInterval(clock));
</script>

<template>
  <div class="border-t border-default px-3 py-2.5">
    <div v-if="!installed" class="flex items-center gap-3">
      <span
        class="flex size-8 shrink-0 items-center justify-center rounded-full bg-elevated text-dimmed ring-1 ring-default"
      >
        <UIcon name="i-lucide-power" class="size-4" />
      </span>
      <div class="min-w-0 flex-1">
        <p class="text-sm font-medium text-highlighted">Local server</p>
        <p class="truncate text-xs text-muted">Install SpacetimeDB to run one here</p>
      </div>
      <UButton size="xs" color="neutral" variant="soft" to="/installations" @click="emit('close')"
        >Install</UButton
      >
    </div>

    <template v-else>
      <div class="flex items-center gap-3">
        <button
          type="button"
          class="flex size-8 shrink-0 items-center justify-center rounded-full ring-1 transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary disabled:cursor-not-allowed"
          :class="[ring, !canToggle && !serverAction && 'opacity-60']"
          :disabled="!canToggle"
          :title="toggleTitle"
          :aria-label="toggleTitle"
          :aria-pressed="owned"
          @click="toggle"
        >
          <UIcon v-if="serverAction" name="i-lucide-loader-circle" class="size-4 animate-spin" />
          <UIcon
            v-else
            name="i-lucide-power"
            class="size-4"
            :class="state === 'starting' && 'animate-pulse'"
          />
        </button>

        <div class="min-w-0 flex-1">
          <p class="text-sm font-medium text-highlighted">Local server</p>
          <p
            class="truncate text-xs"
            :class="[detail.class, state === 'running' && !serverAction && 'font-mono']"
            :title="serverStatus?.exitMessage ?? undefined"
          >
            {{ detail.text }}
          </p>
        </div>

        <div class="flex shrink-0 items-center gap-0.5">
          <template v-if="owned">
            <UButton
              v-if="state === 'running'"
              icon="i-lucide-copy"
              color="neutral"
              variant="ghost"
              size="xs"
              aria-label="Copy server address"
              title="Copy address"
              @click="copyAddress"
            />
            <UButton
              icon="i-lucide-rotate-cw"
              color="neutral"
              variant="ghost"
              size="xs"
              :disabled="Boolean(serverAction || versionOperation)"
              aria-label="Restart server"
              title="Restart"
              @click="restartServer"
            />
          </template>
          <UButton
            v-if="serverStatus?.output.length"
            icon="i-lucide-scroll-text"
            color="neutral"
            variant="ghost"
            size="xs"
            aria-label="Show server output"
            title="Server output"
            @click="showOutput"
          />
          <UButton
            v-if="!owned"
            icon="i-lucide-sliders-horizontal"
            color="neutral"
            :variant="showOptions ? 'soft' : 'ghost'"
            size="xs"
            :disabled="Boolean(serverAction)"
            aria-label="Server options"
            title="Options"
            :aria-expanded="showOptions"
            @click="showOptions = !showOptions"
          />
        </div>
      </div>

      <div v-if="showOptions && !owned" class="mt-2.5 space-y-3 rounded-md bg-elevated/50 p-2.5">
        <div class="flex items-center justify-between gap-3">
          <div>
            <p class="text-xs font-medium text-highlighted">Port</p>
            <p class="text-xs" :class="portValid ? 'text-muted' : 'text-error'">
              {{ portValid ? "1024–65535" : "Use 1024–65535" }}
            </p>
          </div>
          <UInput
            v-model.number="serverOptions.port"
            type="number"
            :min="1024"
            :max="65535"
            size="xs"
            class="w-24"
            :color="portValid ? undefined : 'error'"
            aria-label="Port"
          />
        </div>
        <USwitch
          v-model="serverOptions.inMemory"
          size="xs"
          label="In memory"
          description="Data is lost when the server stops."
        />
        <USwitch
          v-model="serverOptions.allowRemote"
          size="xs"
          label="Allow other devices"
          description="Listen on every network interface."
        />
      </div>

      <p
        v-if="serverError"
        class="mt-2 line-clamp-3 text-xs break-words text-error"
        :title="serverError"
      >
        {{ serverError }}
      </p>
    </template>
  </div>
</template>
