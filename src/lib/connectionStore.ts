import { computed, ref } from "vue";
import {
  clearSelectedConnectionId,
  getSelectedConnectionId,
  listConnections,
  setSelectedConnectionId,
  testConnection,
  type ConnectionProfile,
} from "./spacetime";

// A single reactive source of truth for the connection list, the active
// selection, and whether that selection is actually reachable. Shared across
// the sidebar switcher and every page. The selection still persists to
// localStorage via the spacetime helpers so a reload restores it, but the
// reactive refs let the UI react the moment it changes.
export type ConnectionStatus = "idle" | "checking" | "connected" | "error";

const connections = ref<ConnectionProfile[]>([]);
const selectedId = ref<string | null>(getSelectedConnectionId());
const loading = ref(false);
const loadError = ref("");

// Start optimistic when a selection was restored so navigation is not blocked
// during the first reachability check on launch.
const status = ref<ConnectionStatus>(selectedId.value ? "checking" : "idle");
const statusMessage = ref("");

const selectedConnection = computed(
  () => connections.value.find((connection) => connection.id === selectedId.value) ?? null,
);

// The green-dot state: true only once a connection test has actually succeeded.
const isConnected = computed(() => status.value === "connected");

// The navigation gate. Kept permissive while a check is in flight so a reload
// on, say, the tables page is not bounced to the connections page mid-check;
// only a confirmed failure or no selection at all locks the rest of the app.
const canAccess = computed(
  () => selectedId.value !== null && status.value !== "error",
);

// Guards against an earlier, slower check overwriting the result of a newer one.
let checkToken = 0;

async function checkConnection() {
  const id = selectedId.value;

  if (!id || !connections.value.some((connection) => connection.id === id)) {
    status.value = "idle";
    statusMessage.value = "";
    return;
  }

  const token = ++checkToken;
  status.value = "checking";
  statusMessage.value = "";

  try {
    const result = await testConnection({ id });
    if (token !== checkToken) return;
    status.value = result.ok ? "connected" : "error";
    statusMessage.value = result.message;
  } catch (err) {
    if (token !== checkToken) return;
    status.value = "error";
    statusMessage.value = String(err);
  }
}

function selectConnection(id: string | null) {
  selectedId.value = id;
  if (id) {
    setSelectedConnectionId(id);
  } else {
    clearSelectedConnectionId();
  }
  checkConnection();
}

async function loadConnections() {
  loading.value = true;
  loadError.value = "";

  try {
    connections.value = await listConnections();

    // The stored id can point at a profile that no longer exists, which would
    // otherwise leave the workspace pinned to a connection it cannot resolve.
    if (!connections.value.some((connection) => connection.id === selectedId.value)) {
      selectConnection(connections.value[0]?.id ?? null);
    } else {
      // Selection is still valid; re-verify it is reachable.
      checkConnection();
    }
  } catch (err) {
    loadError.value = String(err);
    throw err;
  } finally {
    loading.value = false;
  }
}

export function useConnections() {
  return {
    connections,
    selectedId,
    selectedConnection,
    status,
    statusMessage,
    isConnected,
    canAccess,
    loading,
    loadError,
    loadConnections,
    selectConnection,
    checkConnection,
  };
}
