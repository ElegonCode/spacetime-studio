import { ref, watch } from "vue";
import {
  errorMessage, getLocalServerStatus, startLocalServer, stopLocalServer,
  type LocalServerOptions, type LocalServerStatus,
} from "./ecosystem";

const OPTIONS_KEY = "spacetime-studio:local-server-options";
const defaultOptions: LocalServerOptions = { port: 3000, inMemory: false, allowRemote: false };

function loadOptions(): LocalServerOptions {
  try {
    const stored = JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? "null");
    if (stored && typeof stored === "object") return { ...defaultOptions, ...stored };
  } catch { /* Fall back to defaults. */ }
  return { ...defaultOptions };
}

// Module state so the server status and pending action survive navigation.
export const serverOptions = ref<LocalServerOptions>(loadOptions());
export const serverStatus = ref<LocalServerStatus | null>(null);
export const serverAction = ref<"start" | "stop" | "restart" | null>(null);
export const serverError = ref<string | null>(null);

watch(serverOptions, (options) => {
  try { localStorage.setItem(OPTIONS_KEY, JSON.stringify(options)); } catch { /* Preferences are optional. */ }
}, { deep: true });

export const validPort = (port: number) => Number.isInteger(port) && port >= 1024 && port <= 65535;

let refreshing = false;
export async function refreshServerStatus() {
  if (refreshing || serverAction.value) return;
  refreshing = true;
  try {
    const port = serverOptions.value.port;
    serverStatus.value = await getLocalServerStatus(validPort(port) ? port : undefined);
  } catch (error) { serverError.value = errorMessage(error); }
  finally { refreshing = false; }
}

async function run(action: "start" | "stop" | "restart", task: () => Promise<LocalServerStatus>) {
  if (serverAction.value) return;
  serverAction.value = action;
  serverError.value = null;
  try { serverStatus.value = await task(); }
  catch (error) { serverError.value = errorMessage(error); }
  finally { serverAction.value = null; }
  await refreshServerStatus();
}

export const startServer = () => run("start", () => startLocalServer({ ...serverOptions.value }));
export const stopServer = () => run("stop", stopLocalServer);
export const restartServer = () => run("restart", async () => {
  await stopLocalServer();
  return startLocalServer({ ...serverOptions.value });
});

// Opened from the connection picker, which unmounts its popover content.
export const serverOutputOpen = ref(false);

let pollTimer: ReturnType<typeof setInterval> | null = null;
let pollInterval: number | null = null;
/** Re-checks the server every `ms`, or stops checking when `ms` is null. */
export function setServerPolling(ms: number | null) {
  if (ms === pollInterval) return;
  if (pollTimer) clearInterval(pollTimer);
  pollTimer = null;
  pollInterval = ms;
  if (ms === null) return;
  void refreshServerStatus();
  pollTimer = setInterval(() => void refreshServerStatus(), ms);
}
