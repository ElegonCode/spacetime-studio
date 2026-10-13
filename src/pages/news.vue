<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { clearPageRefreshHandler, setPageRefreshHandler, type PageRefreshReason } from "../lib/pageActions";
import { newsArticles, newsCheckedAt, newsError, newsLoading, refreshNews } from "../lib/newsStore";
import { errorMessage } from "../lib/ecosystem";

const failedImages = ref(new Set<string>());
const linkError = ref<string | null>(null);
let timer: ReturnType<typeof setInterval> | undefined;
const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });
const timeFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

async function openArticle(url: string) {
  linkError.value = null;
  try { await openUrl(url); } catch (error) { linkError.value = errorMessage(error); }
}
const refresh = async (reason?: PageRefreshReason) => {
  if (reason !== "connection-change") await refreshNews();
};
function checkNews() {
  if (document.visibilityState === "visible") void refreshNews(false);
}
onMounted(() => {
  setPageRefreshHandler(refresh);
  void refreshNews(false);
  timer = setInterval(checkNews, 15 * 60 * 1000);
  document.addEventListener("visibilitychange", checkNews);
});
onUnmounted(() => {
  clearPageRefreshHandler(refresh);
  clearInterval(timer);
  document.removeEventListener("visibilitychange", checkNews);
});
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-7xl space-y-6">
      <div class="flex flex-wrap items-start justify-between gap-4">
        <div>
          <h1 class="text-lg font-semibold text-highlighted">SpacetimeDB news</h1>
          <p class="mt-1 text-sm text-muted">Articles and announcements, straight from the official blog.</p>
          <p v-if="newsCheckedAt" class="mt-2 text-xs text-dimmed">Updated {{ timeFormat.format(newsCheckedAt) }} · Checks for new articles every 15 minutes while this page is open.</p>
        </div>
        <UButton icon="i-lucide-external-link" color="neutral" variant="soft" @click="openArticle('https://spacetimedb.com/blog')">Official blog</UButton>
      </div>

      <UAlert v-if="newsError" color="error" variant="subtle" icon="i-lucide-wifi-off" :title="newsArticles.length ? 'Could not refresh news' : 'Could not load news'" :description="newsError">
        <template #actions><UButton size="xs" color="neutral" variant="soft" :loading="newsLoading" @click="refreshNews()">Retry</UButton></template>
      </UAlert>
      <UAlert v-if="linkError" color="error" title="Could not open article" :description="linkError" />

      <div v-if="newsLoading && !newsArticles.length" class="grid gap-5 sm:grid-cols-2 xl:grid-cols-3" role="status" aria-label="Loading news">
        <UCard v-for="n in 6" :key="n"><USkeleton class="h-40 w-full" /><USkeleton class="mt-5 h-5 w-3/4" /><USkeleton class="mt-3 h-16 w-full" /></UCard>
      </div>
      <div v-else-if="!newsArticles.length && !newsError" class="rounded-lg border border-default p-12 text-center text-sm text-muted">No articles are available in the feed yet.</div>
      <div v-else class="grid gap-5 sm:grid-cols-2 xl:grid-cols-3" :aria-busy="newsLoading">
        <article v-for="article in newsArticles" :key="article.url" class="min-w-0">
          <a :href="article.url" class="group flex h-full flex-col overflow-hidden rounded-lg border border-default bg-default transition-colors hover:border-accented focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-primary" @click.prevent="openArticle(article.url)">
            <div class="flex h-44 shrink-0 items-center justify-center overflow-hidden border-b border-default bg-elevated">
              <img v-if="article.image && !failedImages.has(article.url)" :src="article.image" alt="" loading="lazy" referrerpolicy="no-referrer" class="h-full w-full object-contain" @error="failedImages.add(article.url)" />
              <UIcon v-else name="i-lucide-newspaper" class="size-12 text-dimmed" />
            </div>
            <div class="flex flex-1 flex-col p-5">
              <time v-if="article.publishedAt" :datetime="article.publishedAt" class="text-xs text-muted">{{ dateFormat.format(new Date(article.publishedAt)) }}</time>
              <h2 class="mt-2 text-base font-semibold text-highlighted group-hover:text-primary">{{ article.title }}</h2>
              <p class="mt-3 line-clamp-3 text-sm leading-relaxed text-muted">{{ article.description }}</p>
              <div class="mt-auto flex items-center justify-between gap-3 pt-5 text-xs text-muted">
                <span>{{ article.author }}</span><UIcon name="i-lucide-arrow-up-right" class="size-4 shrink-0" aria-hidden="true" />
              </div>
            </div>
          </a>
        </article>
      </div>
    </div>
  </div>
</template>
