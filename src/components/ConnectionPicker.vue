<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  clearSelectedConnectionId,
  deleteConnection,
  getSelectedConnectionId,
  listConnections,
  saveConnection,
  setSelectedConnectionId,
  testConnection,
  type ConnectionProfile,
} from "../lib/spacetime";
import { clearPageRefreshHandler, setPageRefreshHandler } from "../lib/pageActions";

const emit = defineEmits<{
  changed: [connection: ConnectionProfile | null];
}>();

const MAINCLOUD_URL = "https://maincloud.spacetimedb.com";
const LOCALHOST_URL = "http://localhost:3000";

const hostModeOptions = [
  { label: "Local server", value: "local" },
  { label: "Hosted in Maincloud", value: "maincloud" },
  { label: "Self-hosted (Railway, Fly, VPS)", value: "custom" },
];

type HostMode = "local" | "maincloud" | "custom";

const connections = ref<ConnectionProfile[]>([]);
const selectedId = ref<string | undefined>(getSelectedConnectionId() ?? undefined);
const loading = ref(false);
const saving = ref(false);
const testing = ref(false);
const deleting = ref(false);
const error = ref("");
const status = ref("");
const editorOpen = ref(false);
const pendingDelete = ref<ConnectionProfile | null>(null);

const form = ref({
  id: "",
  name: "Localhost",
  baseUrl: LOCALHOST_URL,
  database: "",
  token: "",
});
const hostMode = ref<HostMode>("local");

const selectedConnection = computed(
  () => connections.value.find((connection) => connection.id === selectedId.value) ?? null,
);

const isEditing = computed(() => Boolean(form.value.id));

const editingHasToken = computed(
  () =>
    connections.value.find((connection) => connection.id === form.value.id)?.hasToken ?? false,
);

const selectedBaseUrl = computed(() => {
  if (hostMode.value === "maincloud") return MAINCLOUD_URL;
  if (hostMode.value === "local") return LOCALHOST_URL;
  return form.value.baseUrl;
});

const tokenHelp = computed(() =>
  hostMode.value === "maincloud"
    ? "Optional. Run `spacetime login`, then `spacetime login show --token` to copy it."
    : "Optional for public databases. Run `spacetime login --server-issued-login <server>`, then `spacetime login show --token` to copy the token your own server issued.",
);

function hostModeForUrl(baseUrl: string): HostMode {
  if (baseUrl === MAINCLOUD_URL) return "maincloud";
  if (baseUrl === LOCALHOST_URL) return "local";
  return "custom";
}

function hostLabel(baseUrl: string) {
  if (baseUrl === MAINCLOUD_URL) return "Maincloud";
  return baseUrl.replace(/^https?:\/\//, "");
}

// Don't leave a preset URL sitting in the custom field once the user opts out of the preset.
watch(hostMode, (mode) => {
  if (mode === "custom" && hostModeForUrl(form.value.baseUrl) !== "custom") {
    form.value.baseUrl = "";
  }
});

function resetForm() {
  form.value = {
    id: "",
    name: "Localhost",
    baseUrl: LOCALHOST_URL,
    database: "",
    token: "",
  };
  hostMode.value = "local";
}

async function load() {
  loading.value = true;
  error.value = "";

  try {
    connections.value = await listConnections();

    // The stored id can point at a profile that no longer exists, which used to leave
    // the picker stuck on a connection it could not resolve.
    if (!connections.value.some((connection) => connection.id === selectedId.value)) {
      selectedId.value = connections.value[0]?.id;
      if (selectedId.value) {
        setSelectedConnectionId(selectedId.value);
      } else {
        clearSelectedConnectionId();
      }
    }

    emit("changed", selectedConnection.value);
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

function activate(connection: ConnectionProfile) {
  selectedId.value = connection.id;
  setSelectedConnectionId(connection.id);
  error.value = "";
  status.value = "";
  emit("changed", selectedConnection.value);
}

function editConnection(connection: ConnectionProfile) {
  form.value = {
    id: connection.id,
    name: connection.name,
    baseUrl: connection.baseUrl,
    database: connection.database,
    token: "",
  };
  hostMode.value = hostModeForUrl(connection.baseUrl);
  error.value = "";
  status.value = "";
  editorOpen.value = true;
}

function newConnection() {
  resetForm();
  error.value = "";
  status.value = "";
  editorOpen.value = true;
}

async function save() {
  if (saving.value) return;

  saving.value = true;
  error.value = "";
  status.value = "";

  try {
    const connection = await saveConnection({
      id: form.value.id || undefined,
      name: form.value.name,
      baseUrl: selectedBaseUrl.value,
      database: form.value.database,
      token: form.value.token || undefined,
    });
    const wasNew = !form.value.id;
    form.value.id = connection.id;
    form.value.token = "";
    selectedId.value = connection.id;
    setSelectedConnectionId(connection.id);
    editorOpen.value = false;
    await load();
    status.value = wasNew
      ? `Added ${connection.name}.`
      : `Saved changes to ${connection.name}.`;
  } catch (err) {
    error.value = String(err);
  } finally {
    saving.value = false;
  }
}

async function test() {
  testing.value = true;
  error.value = "";
  status.value = "";

  try {
    const result = await testConnection({
      id: form.value.id || undefined,
      baseUrl: selectedBaseUrl.value,
      database: form.value.database,
      token: form.value.token || undefined,
    });
    status.value = result.databaseIdentity
      ? `${result.message}. Database identity ${result.databaseIdentity}.`
      : result.message;
  } catch (err) {
    error.value = String(err);
  } finally {
    testing.value = false;
  }
}

function askDelete(connection: ConnectionProfile) {
  pendingDelete.value = connection;
}

async function confirmDelete() {
  const connection = pendingDelete.value;
  if (!connection || deleting.value) return;

  deleting.value = true;
  error.value = "";
  status.value = "";

  try {
    await deleteConnection(connection.id);
    if (selectedId.value === connection.id) {
      clearSelectedConnectionId();
      selectedId.value = undefined;
    }
    pendingDelete.value = null;
    await load();
    status.value = `Removed ${connection.name}.`;
  } catch (err) {
    error.value = String(err);
  } finally {
    deleting.value = false;
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
  <section class="rounded-lg border border-default bg-default/40 p-4">
    <div class="mb-4 flex items-center justify-between gap-3">
      <div>
        <h2 class="text-base font-semibold text-highlighted">Connections</h2>
        <p class="text-xs text-muted">Select the connection the workspace should use.</p>
      </div>
      <UButton icon="i-lucide-plus" size="sm" @click="newConnection">New</UButton>
    </div>

    <p v-if="loading && connections.length === 0" class="py-6 text-center text-sm text-muted">
      Loading connections...
    </p>

    <div
      v-else-if="connections.length === 0"
      class="rounded-lg border border-dashed border-default px-4 py-8 text-center"
    >
      <p class="text-sm font-medium text-highlighted">No connections yet</p>
      <p class="mt-1 text-xs text-muted">
        Add a local, Maincloud, or self-hosted SpacetimeDB database to get started.
      </p>
      <UButton icon="i-lucide-plus" size="sm" class="mt-4" @click="newConnection">
        Add connection
      </UButton>
    </div>

    <ul v-else class="space-y-2" role="radiogroup" aria-label="Active connection">
      <li
        v-for="connection in connections"
        :key="connection.id"
        class="flex items-stretch gap-1 rounded-lg border transition-colors"
        :class="
          connection.id === selectedId
            ? 'border-primary bg-primary/5'
            : 'border-default hover:bg-elevated/50'
        "
      >
        <button
          type="button"
          role="radio"
          :aria-checked="connection.id === selectedId"
          class="flex min-w-0 flex-1 items-center gap-3 rounded-l-lg px-3 py-2.5 text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-primary"
          @click="activate(connection)"
        >
          <UIcon
            :name="
              connection.id === selectedId ? 'i-lucide-circle-check' : 'i-lucide-circle'
            "
            class="size-4 shrink-0"
            :class="connection.id === selectedId ? 'text-primary' : 'text-muted'"
          />
          <span class="min-w-0 flex-1">
            <span class="flex items-center gap-2">
              <span class="truncate text-sm font-medium text-highlighted">
                {{ connection.name }}
              </span>
              <UBadge v-if="connection.id === selectedId" size="sm" variant="subtle">
                Active
              </UBadge>
            </span>
            <span class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
              <span class="truncate">{{ connection.database }}</span>
              <span aria-hidden="true">&middot;</span>
              <span class="truncate">{{ hostLabel(connection.baseUrl) }}</span>
              <span
                class="flex shrink-0 items-center"
                :title="connection.hasToken ? 'Stored token' : 'Anonymous or public access'"
              >
                <UIcon
                  :name="connection.hasToken ? 'i-lucide-key-round' : 'i-lucide-globe'"
                  class="size-3"
                />
                <span class="sr-only">
                  {{ connection.hasToken ? "Stored token" : "Anonymous or public access" }}
                </span>
              </span>
            </span>
          </span>
        </button>

        <div class="flex shrink-0 items-center gap-1 pr-2">
          <UButton
            icon="i-lucide-pencil"
            color="neutral"
            variant="ghost"
            size="sm"
            aria-label="Edit connection"
            @click="editConnection(connection)"
          />
          <UButton
            icon="i-lucide-trash-2"
            color="error"
            variant="ghost"
            size="sm"
            aria-label="Delete connection"
            @click="askDelete(connection)"
          />
        </div>
      </li>
    </ul>

    <UAlert
      v-if="error && !editorOpen"
      color="error"
      variant="subtle"
      class="mt-4"
      :description="error"
    />
    <UAlert
      v-if="status && !editorOpen"
      color="success"
      variant="subtle"
      class="mt-4"
      :description="status"
    />

    <USlideover
      v-model:open="editorOpen"
      :title="isEditing ? 'Edit Connection' : 'New Connection'"
      description="SpacetimeDB HTTP API profile"
      side="right"
      :ui="{ content: 'sm:max-w-xl', body: 'min-h-0 flex-1', footer: 'shrink-0' }"
    >
      <template #body>
        <form class="space-y-4" @submit.prevent="save">
          <UFormField label="Name">
            <UInput v-model="form.name" class="w-full" />
          </UFormField>
          <UFormField label="Database name or identity">
            <UInput v-model="form.database" class="w-full" placeholder="my-database" />
          </UFormField>
          <UFormField label="Host">
            <USelect v-model="hostMode" :items="hostModeOptions" class="w-full" />
          </UFormField>
          <UFormField
            v-if="hostMode === 'custom'"
            label="Host URL"
            help="Your server's public domain. https:// is assumed if you leave the scheme off."
          >
            <UInput
              v-model="form.baseUrl"
              class="w-full"
              placeholder="my-app.up.railway.app"
            />
          </UFormField>
          <UFormField v-else label="Host URL">
            <UInput :model-value="selectedBaseUrl" class="w-full" disabled />
          </UFormField>
          <UFormField label="Auth token" :help="tokenHelp">
            <UInput
              v-model="form.token"
              class="w-full"
              type="password"
              :placeholder="
                isEditing && editingHasToken
                  ? 'Leave blank to keep the stored token'
                  : 'Stored in the OS credential manager'
              "
            />
          </UFormField>

          <UAlert v-if="error" color="error" variant="subtle" :description="error" />
          <UAlert v-if="status" color="success" variant="subtle" :description="status" />
        </form>
      </template>

      <template #footer="{ close }">
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="close">Cancel</UButton>
          <UButton
            icon="i-lucide-plug-zap"
            color="neutral"
            variant="soft"
            :loading="testing"
            @click="test"
          >
            Test
          </UButton>
          <UButton icon="i-lucide-save" :loading="saving" @click="save">Save</UButton>
        </div>
      </template>
    </USlideover>

    <UModal
      :open="pendingDelete !== null"
      title="Delete connection"
      :description="`Remove ${pendingDelete?.name ?? ''} and its stored token from this computer? The database itself is not affected.`"
      @update:open="pendingDelete = null"
    >
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="pendingDelete = null">
            Cancel
          </UButton>
          <UButton color="error" :loading="deleting" @click="confirmDelete">Delete</UButton>
        </div>
      </template>
    </UModal>
  </section>
</template>
