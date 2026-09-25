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

// Last known reachability of every saved profile, not just the active one, so
// the sidebar picker can show which servers are online before switching.
export type Reachability = { status: ConnectionStatus; message: string };
const reachability = ref<Record<string, Reachability>>({});

function setReachability(id: string, next: Reachability) {
  reachability.value = { ...reachability.value, [id]: next };
}

const selectedConnection = computed(
  () => connections.value.find((connection) => connection.id === selectedId.value) ?? null,
);

// The green-dot state: true only once a connection test has actually succeeded.
const isConnected = computed(() => status.value === "connected");

// Whether the active selection's most recent completed check failed. Unlike
// status, it survives a re-check in flight, so re-pinging a broken connection
// does not briefly unlock the workspace. Cleared when the selection changes.
const lastCheckFailed = ref(false);

// The navigation gate. Kept permissive while the first check of a selection is
// in flight so a reload on, say, the tables page is not bounced to the landing
// page mid-check; only a confirmed failure or no selection at all locks the app.
const canAccess = computed(
  () => selectedId.value !== null && !lastCheckFailed.value,
);

// Guards against an earlier, slower check overwriting the result of a newer one.
let checkToken = 0;

async function checkConnection() {
  const id = selectedId.value;

  if (!id || !connections.value.some((connection) => connection.id === id)) {
    status.value = "idle";
    statusMessage.value = "";
    lastCheckFailed.value = false;
    return;
  }

  const token = ++checkToken;
  status.value = "checking";
  setReachability(id, { status: "checking", message: "" });

  const result = await probe(id);
  if (token !== checkToken) return;
  setReachability(id, result);
  status.value = result.status;
  statusMessage.value = result.message;
  lastCheckFailed.value = result.status === "error";
}

async function probe(id: string): Promise<Reachability> {
  try {
    const result = await testConnection({ id });
    return { status: result.ok ? "connected" : "error", message: result.message };
  } catch (err) {
    return { status: "error", message: String(err) };
  }
}

// Ping every saved profile in parallel. The active one goes through
// checkConnection so the workspace gate stays in sync with what the list shows.
let pingToken = 0;

async function pingAll() {
  const token = ++pingToken;
  await Promise.all(
    connections.value.map(async ({ id }) => {
      if (id === selectedId.value) return checkConnection();
      setReachability(id, { status: "checking", message: "" });
      const result = await probe(id);
      if (token === pingToken) setReachability(id, result);
    }),
  );
}

function selectConnection(id: string | null) {
  if (id !== selectedId.value) {
    lastCheckFailed.value = false;
    statusMessage.value = "";
  }
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

    const ids = new Set(connections.value.map((connection) => connection.id));
    reachability.value = Object.fromEntries(
      Object.entries(reachability.value).filter(([id]) => ids.has(id)),
    );

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
    lastCheckFailed,
    loading,
    loadError,
    reachability,
    loadConnections,
    pingAll,
    selectConnection,
    checkConnection,
  };
}
