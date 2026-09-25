<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  saveConnection,
  testConnection,
  type ConnectionProfile,
} from "../lib/spacetime";
import { useConnections } from "../lib/connectionStore";
import { hostModeForUrl, LOCALHOST_URL, MAINCLOUD_URL, type HostMode } from "../lib/hosts";

// Slide-over form for adding a profile or editing an existing one. Passing a
// connection edits it; passing null starts a new one.
const props = defineProps<{ connection: ConnectionProfile | null }>();
const open = defineModel<boolean>("open", { required: true });

const hostModeOptions = [
  { label: "Local server", value: "local" },
  { label: "Hosted in Maincloud", value: "maincloud" },
  { label: "Self-hosted (Railway, Fly, VPS)", value: "custom" },
];

const { loadConnections, selectConnection } = useConnections();
const toast = useToast();

const saving = ref(false);
const testing = ref(false);
const error = ref("");
const status = ref("");

const form = ref({
  id: "",
  name: "Localhost",
  baseUrl: LOCALHOST_URL,
  database: "",
  token: "",
});
const hostMode = ref<HostMode>("local");

const isEditing = computed(() => Boolean(form.value.id));
const editingHasToken = computed(() => props.connection?.hasToken ?? false);

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

// Reset the form each time the slide-over opens so it reflects the target.
watch(open, (isOpen) => {
  if (!isOpen) return;
  const connection = props.connection;
  form.value = connection
    ? {
        id: connection.id,
        name: connection.name,
        baseUrl: connection.baseUrl,
        database: connection.database,
        token: "",
      }
    : { id: "", name: "Localhost", baseUrl: LOCALHOST_URL, database: "", token: "" };
  hostMode.value = hostModeForUrl(form.value.baseUrl);
  error.value = "";
  status.value = "";
});

// Don't leave a preset URL sitting in the custom field once the user opts out of the preset.
watch(hostMode, (mode) => {
  if (mode === "custom" && hostModeForUrl(form.value.baseUrl) !== "custom") {
    form.value.baseUrl = "";
  }
});

async function save() {
  if (saving.value) return;

  saving.value = true;
  error.value = "";
  status.value = "";

  try {
    const wasNew = !form.value.id;
    const connection = await saveConnection({
      id: form.value.id || undefined,
      name: form.value.name,
      baseUrl: selectedBaseUrl.value,
      database: form.value.database,
      token: form.value.token || undefined,
    });
    await loadConnections();
    // Adding a connection makes it the active one so the workspace unlocks
    // immediately; editing leaves the current selection untouched.
    if (wasNew) {
      selectConnection(connection.id);
    }
    open.value = false;
    toast.add({
      title: wasNew ? `Added ${connection.name}` : `Saved ${connection.name}`,
      color: "success",
      icon: "i-lucide-check",
    });
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
</script>

<template>
  <USlideover
    v-model:open="open"
    :title="isEditing ? 'Edit connection' : 'New connection'"
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
          <UInput v-model="form.baseUrl" class="w-full" placeholder="my-app.up.railway.app" />
        </UFormField>
        <UFormField v-else label="Host URL">
          <UInput :model-value="selectedBaseUrl" class="w-full" disabled />
        </UFormField>
        <UFormField label="Auth token">
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

        <UAlert color="neutral" variant="subtle" icon="i-lucide-info" :description="tokenHelp" />

        <UAlert v-if="error" color="error" variant="subtle" :description="error" />
        <UAlert v-if="status" color="success" variant="subtle" :description="status" />

        <!-- Lets Enter submit from any field. -->
        <button type="submit" class="hidden" />
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
</template>
