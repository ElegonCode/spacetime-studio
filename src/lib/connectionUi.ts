import { ref } from "vue";
import type { ConnectionProfile } from "./spacetime";

// The connection editor slide-over is mounted once by the sidebar picker, but
// other surfaces (like the empty workspace) need to be able to open it too.
export const editorOpen = ref(false);
export const editorTarget = ref<ConnectionProfile | null>(null);

export function openConnectionEditor(connection: ConnectionProfile | null = null) {
  editorTarget.value = connection;
  editorOpen.value = true;
}
