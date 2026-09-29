import { computed, ref, shallowRef } from "vue";
import { check, type Update } from "@tauri-apps/plugin-updater";

const storageKey = "spacetime-studio.last-update-check";
const statusStorageKey = "spacetime-studio.last-update-check-status";
const cooldownMs = 60_000;
const savedCheck = Number(localStorage.getItem(storageKey));

// Tauri's Update class uses JavaScript private fields; a deep Vue ref proxy
// breaks its methods, so keep the instance unproxied.
export const availableUpdate = shallowRef<Update | null>(null);
export const checkingForUpdate = ref(false);
export const updateCheckFailed = ref(localStorage.getItem(statusStorageKey) === "failed");
export const lastUpdateCheck = ref<Date | null>(
  Number.isFinite(savedCheck) && savedCheck > 0 ? new Date(savedCheck) : null,
);
const now = ref(Date.now());
window.setInterval(() => {
  now.value = Date.now();
}, 1000);

export const updateCheckCooldown = computed(() => {
  if (!lastUpdateCheck.value) return 0;
  return Math.max(
    0,
    Math.ceil((lastUpdateCheck.value.getTime() + cooldownMs - now.value) / 1000),
  );
});

export async function checkForUpdate() {
  if (checkingForUpdate.value || updateCheckCooldown.value > 0) return;

  checkingForUpdate.value = true;
  updateCheckFailed.value = false;
  try {
    availableUpdate.value = await check();
  } catch {
    updateCheckFailed.value = true;
  } finally {
    const checkedAt = Date.now();
    lastUpdateCheck.value = new Date(checkedAt);
    localStorage.setItem(storageKey, String(checkedAt));
    localStorage.setItem(statusStorageKey, updateCheckFailed.value ? "failed" : "success");
    now.value = checkedAt;
    checkingForUpdate.value = false;
  }
}
