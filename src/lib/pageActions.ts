import { ref, shallowRef } from "vue";

export type PageRefreshReason = "manual" | "connection-change";
type PageRefreshHandler = (reason?: PageRefreshReason) => Promise<void> | void;

export const pageRefreshHandler = shallowRef<PageRefreshHandler | null>(null);
export const pageRefreshLoading = ref(false);

export function setPageRefreshHandler(handler: PageRefreshHandler) {
  pageRefreshHandler.value = handler;
}

export function clearPageRefreshHandler(handler: PageRefreshHandler) {
  if (pageRefreshHandler.value === handler) {
    pageRefreshHandler.value = null;
  }
}

export async function runPageRefresh(reason: PageRefreshReason = "manual") {
  const handler = pageRefreshHandler.value;

  if (!handler || pageRefreshLoading.value) return;

  pageRefreshLoading.value = true;

  try {
    await handler(reason);
  } finally {
    pageRefreshLoading.value = false;
  }
}
