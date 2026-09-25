<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { deleteConnection, type ConnectionProfile } from "../lib/spacetime";
import { useConnections, type Reachability } from "../lib/connectionStore";
import { editorOpen, editorTarget, openConnectionEditor } from "../lib/connectionUi";
import { hostLabel } from "../lib/hosts";
import ConnectionEditor from "./ConnectionEditor.vue";

// The sidebar's connection switcher and manager in one: pick the active
// profile, see which servers are reachable, and add, edit, or remove profiles.
defineProps<{ collapsed?: boolean }>();

const {
  connections,
  selectedId,
  selectedConnection,
  status,
  statusMessage,
  reachability,
  loading,
  loadConnections,
  selectConnection,
  pingAll,
} = useConnections();
const toast = useToast();

const open = ref(false);
const pendingDelete = ref<ConnectionProfile | null>(null);
const deleting = ref(false);

const pinging = computed(() =>
  connections.value.some(({ id }) => reachability.value[id]?.status === "checking"),
);

// Refresh every server's status whenever the picker opens, so the list shows
// what is actually reachable right now.
watch(open, (isOpen) => {
  if (isOpen) pingAll();
});

type Badge = { label: string; dot: string; text: string };

// A failure to reach the host at all reads as "Offline"; anything after that
// (missing database, rejected token) means the server is up but unusable.
function badgeFor(entry: Reachability | undefined): Badge {
  switch (entry?.status) {
    case "connected":
      return { label: "Online", dot: "bg-green-500", text: "text-green-400" };
    case "checking":
      return { label: "Checking", dot: "bg-amber-500 animate-pulse", text: "text-muted" };
    case "error":
      return /^(Could not reach|Host responded)/.test(entry.message)
        ? { label: "Offline", dot: "bg-red-500", text: "text-red-400" }
        : { label: "Error", dot: "bg-orange-500", text: "text-orange-400" };
    default:
      return { label: "Unknown", dot: "bg-neutral-500", text: "text-muted" };
  }
}

const activeBadge = computed(() =>
  selectedId.value
    ? badgeFor({ status: status.value, message: statusMessage.value })
    : { label: "No connection", dot: "bg-neutral-500", text: "text-muted" },
);

const activeTitle = computed(() =>
  status.value === "error" && statusMessage.value
    ? `${activeBadge.value.label}: ${statusMessage.value}`
    : activeBadge.value.label,
);

function choose(connection: ConnectionProfile) {
  if (connection.id !== selectedId.value) selectConnection(connection.id);
  open.value = false;
}

function edit(connection: ConnectionProfile | null) {
  open.value = false;
  openConnectionEditor(connection);
}

function askDelete(connection: ConnectionProfile) {
  open.value = false;
  pendingDelete.value = connection;
}

async function confirmDelete() {
  const connection = pendingDelete.value;
  if (!connection || deleting.value) return;

  deleting.value = true;
  try {
    await deleteConnection(connection.id);
    pendingDelete.value = null;
    await loadConnections();
    toast.add({ title: `Removed ${connection.name}`, icon: "i-lucide-trash-2" });
  } catch (err) {
    toast.add({ title: "Could not remove connection", description: String(err), color: "error" });
  } finally {
    deleting.value = false;
  }
}
</script>

<template>
  <UPopover
    v-model:open="open"
    :content="{
      side: collapsed ? 'right' : 'bottom',
      align: 'start',
      // Focusing the first row on open would reveal its edit/delete actions.
      onOpenAutoFocus: (event: Event) => event.preventDefault(),
    }"
  >
    <UButton
      v-if="!collapsed"
      color="neutral"
      variant="outline"
      block
      class="justify-start gap-2.5 px-2.5 py-2"
      :title="activeTitle"
    >
      <span class="size-2 shrink-0 rounded-full" :class="activeBadge.dot" />
      <span class="min-w-0 flex-1 text-left">
        <span class="block truncate text-sm font-medium text-highlighted">
          {{ selectedConnection?.name ?? "No connection" }}
        </span>
        <span class="block truncate text-xs font-normal text-muted">
          {{
            selectedConnection
              ? selectedConnection.database
              : connections.length
                ? "Pick a connection"
                : "Add a connection"
          }}
        </span>
      </span>
      <UIcon name="i-lucide-chevrons-up-down" class="size-4 shrink-0 text-dimmed" />
    </UButton>
    <UButton
      v-else
      color="neutral"
      variant="ghost"
      square
      class="mx-auto flex"
      :title="activeTitle"
      aria-label="Connections"
    >
      <span class="size-2.5 rounded-full" :class="activeBadge.dot" />
    </UButton>

    <template #content>
      <div class="w-80">
        <div class="flex items-center justify-between border-b border-default px-3 py-2">
          <span class="text-xs font-medium uppercase tracking-wide text-muted">Connections</span>
          <UButton
            icon="i-lucide-refresh-cw"
            color="neutral"
            variant="ghost"
            size="xs"
            :loading="pinging"
            :disabled="connections.length === 0"
            aria-label="Re-check servers"
            title="Re-check servers"
            @click="pingAll"
          />
        </div>

        <p
          v-if="loading && connections.length === 0"
          class="px-3 py-6 text-center text-sm text-muted"
        >
          Loading connections...
        </p>

        <div v-else-if="connections.length === 0" class="px-4 py-6 text-center">
          <p class="text-sm font-medium text-highlighted">No connections yet</p>
          <p class="mt-1 text-xs text-muted">
            Add a local, Maincloud, or self-hosted SpacetimeDB database.
          </p>
        </div>

        <ul v-else class="max-h-80 space-y-0.5 overflow-y-auto p-1.5">
          <li v-for="connection in connections" :key="connection.id" class="group relative">
            <button
              type="button"
              class="flex w-full items-center gap-3 rounded-md px-2 py-2 text-left transition-colors hover:bg-elevated focus-visible:bg-elevated focus-visible:outline-none"
              :class="connection.id === selectedId && 'bg-elevated/60'"
              :title="reachability[connection.id]?.message || undefined"
              @click="choose(connection)"
            >
              <span
                class="size-2 shrink-0 rounded-full"
                :class="badgeFor(reachability[connection.id]).dot"
              />
              <span class="min-w-0 flex-1">
                <span class="flex items-center gap-1.5">
                  <span class="truncate text-sm font-medium text-highlighted">
                    {{ connection.name }}
                  </span>
                  <UIcon
                    v-if="connection.id === selectedId"
                    name="i-lucide-check"
                    class="size-3.5 shrink-0 text-primary"
                  />
                </span>
                <span class="mt-0.5 flex items-center gap-1.5 text-xs text-muted">
                  <UIcon
                    :name="connection.hasToken ? 'i-lucide-key-round' : 'i-lucide-globe'"
                    class="size-3 shrink-0"
                    :title="connection.hasToken ? 'Stored token' : 'Anonymous or public access'"
                  />
                  <span class="truncate">
                    {{ connection.database }} &middot; {{ hostLabel(connection.baseUrl) }}
                  </span>
                </span>
              </span>
              <span
                class="shrink-0 text-xs group-hover:invisible group-has-focus-visible:invisible"
                :class="badgeFor(reachability[connection.id]).text"
              >
                {{ badgeFor(reachability[connection.id]).label }}
              </span>
            </button>

            <div
              class="absolute inset-y-0 right-1.5 hidden items-center gap-0.5 group-hover:flex group-has-focus-visible:flex"
            >
              <UButton
                icon="i-lucide-pencil"
                color="neutral"
                variant="ghost"
                size="xs"
                aria-label="Edit connection"
                title="Edit"
                @click="edit(connection)"
              />
              <UButton
                icon="i-lucide-trash-2"
                color="error"
                variant="ghost"
                size="xs"
                aria-label="Delete connection"
                title="Delete"
                @click="askDelete(connection)"
              />
            </div>
          </li>
        </ul>

        <div class="border-t border-default p-1.5">
          <UButton
            icon="i-lucide-plus"
            color="neutral"
            variant="ghost"
            block
            class="justify-start"
            @click="edit(null)"
          >
            Add connection
          </UButton>
        </div>
      </div>
    </template>
  </UPopover>

  <ConnectionEditor v-model:open="editorOpen" :connection="editorTarget" />

  <UModal
    :open="pendingDelete !== null"
    title="Delete connection"
    :description="`Remove ${pendingDelete?.name ?? ''} and its stored token from this computer? The database itself is not affected.`"
    @update:open="pendingDelete = null"
  >
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="soft" @click="() => { pendingDelete = null }">
          Cancel
        </UButton>
        <UButton color="error" :loading="deleting" @click="confirmDelete">Delete</UButton>
      </div>
    </template>
  </UModal>
</template>
