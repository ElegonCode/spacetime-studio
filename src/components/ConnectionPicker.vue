<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
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

const connections = ref<ConnectionProfile[]>([]);
const selectedId = ref<string | undefined>(getSelectedConnectionId() ?? undefined);
const loading = ref(false);
const testing = ref(false);
const error = ref("");
const status = ref("");
const editorOpen = ref(false);

const form = ref({
  id: "",
  name: "Localhost",
  baseUrl: "http://localhost:3000",
  database: "",
  token: "",
});

const selectedConnection = computed(
  () => connections.value.find((connection) => connection.id === selectedId.value) ?? null,
);

const connectionOptions = computed(() =>
  connections.value.map((connection) => ({
    label: `${connection.name} - ${connection.database}`,
    value: connection.id,
  })),
);

function resetForm() {
  form.value = {
    id: "",
    name: "Localhost",
    baseUrl: "http://localhost:3000",
    database: "",
    token: "",
  };
}

async function load() {
  loading.value = true;
  error.value = "";

  try {
    connections.value = await listConnections();
    if (!selectedId.value && connections.value.length > 0) {
      selectedId.value = connections.value[0].id;
      setSelectedConnectionId(selectedId.value);
    }
    emit("changed", selectedConnection.value);
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

function selectCurrent() {
  if (selectedId.value) {
    setSelectedConnectionId(selectedId.value);
  } else {
    clearSelectedConnectionId();
  }
  emit("changed", selectedConnection.value);
}

function editSelected() {
  const connection = selectedConnection.value;
  if (!connection) return;
  form.value = {
    id: connection.id,
    name: connection.name,
    baseUrl: connection.baseUrl,
    database: connection.database,
    token: "",
  };
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
  loading.value = true;
  error.value = "";
  status.value = "";

  try {
    const connection = await saveConnection({
      id: form.value.id || undefined,
      name: form.value.name,
      baseUrl: form.value.baseUrl,
      database: form.value.database,
      token: form.value.token || undefined,
    });
    form.value.id = connection.id;
    form.value.token = "";
    selectedId.value = connection.id;
    setSelectedConnectionId(connection.id);
    status.value = "Connection saved.";
    editorOpen.value = false;
    await load();
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

async function test() {
  testing.value = true;
  error.value = "";
  status.value = "";

  try {
    const result = await testConnection({
      id: form.value.id || undefined,
      baseUrl: form.value.baseUrl,
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

async function removeSelected() {
  const connection = selectedConnection.value;
  if (!connection) return;

  loading.value = true;
  error.value = "";
  status.value = "";

  try {
    await deleteConnection(connection.id);
    if (selectedId.value === connection.id) {
      clearSelectedConnectionId();
      selectedId.value = undefined;
    }
    status.value = "Connection removed.";
    await load();
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
  <section>
    <div class="rounded-lg border border-default bg-default/40 p-4">
      <div class="mb-4 flex items-center justify-between gap-3">
        <h2 class="text-base font-semibold text-highlighted">Connections</h2>
        <div class="flex gap-2">
          <UButton icon="i-lucide-plus" size="sm" @click="newConnection">
            New
          </UButton>
        </div>
      </div>

      <USelect
        v-model="selectedId"
        :items="connectionOptions"
        placeholder="Select a connection"
        class="w-full"
        @update:model-value="selectCurrent"
      />

      <div class="mt-3 flex gap-2">
        <UButton icon="i-lucide-pencil" color="neutral" variant="soft" @click="editSelected">
          Edit
        </UButton>
        <UButton icon="i-lucide-trash-2" color="error" variant="soft" @click="removeSelected">
          Delete
        </UButton>
      </div>

      <div v-if="selectedConnection" class="mt-4 space-y-1 text-sm text-muted">
        <p>{{ selectedConnection.baseUrl }}</p>
        <p>{{ selectedConnection.database }}</p>
        <UBadge :color="selectedConnection.hasToken ? 'success' : 'warning'" variant="subtle">
          {{ selectedConnection.hasToken ? "Stored token" : "Anonymous or public access" }}
        </UBadge>
      </div>

      <UAlert v-if="error && !editorOpen" color="error" variant="subtle" class="mt-4" :description="error" />
      <UAlert v-if="status && !editorOpen" color="success" variant="subtle" class="mt-4" :description="status" />
    </div>

    <USlideover
      v-model:open="editorOpen"
      :title="form.id ? 'Edit Connection' : 'New Connection'"
      description="SpacetimeDB HTTP API profile"
      side="right"
      :ui="{ content: 'sm:max-w-xl', body: 'min-h-0 flex-1', footer: 'shrink-0' }"
    >
      <template #body>
        <form class="space-y-4" @submit.prevent="save">
          <div class="flex justify-end">
            <UBadge variant="subtle">HTTP API</UBadge>
          </div>

          <UFormField label="Name">
            <UInput v-model="form.name" class="w-full" />
          </UFormField>
          <UFormField label="Database name or identity">
            <UInput v-model="form.database" class="w-full" placeholder="my-database" />
          </UFormField>
          <UFormField label="Host URL">
            <UInput v-model="form.baseUrl" class="w-full" placeholder="http://localhost:3000" />
          </UFormField>
          <UFormField label="Bearer token">
            <UInput
              v-model="form.token"
              class="w-full"
              type="password"
              placeholder="Stored in the OS credential manager"
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
          <UButton icon="i-lucide-save" :loading="loading" @click="save">Save</UButton>
        </div>
      </template>
    </USlideover>
  </section>
</template>
