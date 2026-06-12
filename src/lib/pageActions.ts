import { ref, shallowRef } from "vue";

type PageRefreshHandler = () => Promise<void> | void;

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

export async function runPageRefresh() {
  const handler = pageRefreshHandler.value;

  if (!handler || pageRefreshLoading.value) return;

  pageRefreshLoading.value = true;

  try {
    await handler();
  } finally {
    pageRefreshLoading.value = false;
  }
}
