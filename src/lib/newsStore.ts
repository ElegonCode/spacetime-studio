import { ref } from "vue";
import { errorMessage, getSpacetimeNews } from "./ecosystem";

export type NewsArticle = {
  title: string;
  url: string;
  description: string;
  author: string;
  publishedAt: string | null;
  image: string | null;
};

const CACHE_KEY = "spacetime-studio:news";
const REFRESH_INTERVAL = 15 * 60 * 1000;
export const newsArticles = ref<NewsArticle[]>([]);
export const newsLoading = ref(false);
export const newsError = ref<string | null>(null);
export const newsCheckedAt = ref<number | null>(null);

function officialUrl(value: string | null | undefined, article = false): string | null {
  if (!value) return null;
  try {
    const url = new URL(value, "https://spacetimedb.com/blog/");
    return url.protocol === "https:" && url.hostname === "spacetimedb.com"
      && !url.username && !url.password && !url.port
      && (!article || url.pathname.startsWith("/blog/")) ? url.href : null;
  } catch { return null; }
}

function plainText(html: string) {
  const template = document.createElement("template");
  template.innerHTML = html;
  template.content.querySelectorAll("script, style, iframe").forEach((node) => node.remove());
  return (template.content.textContent ?? "").replace(/\s+/g, " ").trim();
}

export function parseNewsFeed(xml: string): NewsArticle[] {
  const feed = new DOMParser().parseFromString(xml, "application/xml");
  if (feed.querySelector("parsererror") || !feed.querySelector("rss > channel")) {
    throw new Error("The SpacetimeDB news feed could not be read.");
  }
  const articles = new Map<string, NewsArticle>();
  for (const item of feed.querySelectorAll("channel > item")) {
    const url = officialUrl(item.querySelector("link")?.textContent?.trim(), true);
    const title = item.querySelector("title")?.textContent?.trim();
    if (!url || !title) continue;
    const content = item.getElementsByTagNameNS("http://purl.org/rss/1.0/modules/content/", "encoded")[0]?.textContent ?? "";
    const template = document.createElement("template");
    template.innerHTML = content;
    const image = [...template.content.querySelectorAll("img")]
      .map((node) => officialUrl(node.getAttribute("src"))).find(Boolean) ?? null;
    const date = Date.parse(item.querySelector("pubDate")?.textContent ?? "");
    articles.set(url, {
      title, url, image,
      description: plainText(item.querySelector("description")?.textContent ?? ""),
      author: item.getElementsByTagNameNS("http://purl.org/dc/elements/1.1/", "creator")[0]?.textContent?.trim() ?? "SpacetimeDB",
      publishedAt: Number.isFinite(date) ? new Date(date).toISOString() : null,
    });
  }
  return [...articles.values()].sort((a, b) => Date.parse(b.publishedAt ?? "") - Date.parse(a.publishedAt ?? "") || 0);
}

try {
  const cache = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
  if (cache && Array.isArray(cache.articles) && typeof cache.checkedAt === "number") {
    newsArticles.value = cache.articles.filter((a: NewsArticle) => a
      && typeof a.title === "string" && typeof a.description === "string" && typeof a.author === "string"
      && officialUrl(a.url, true) && (a.image === null || officialUrl(a.image))
      && (a.publishedAt === null || (typeof a.publishedAt === "string" && Number.isFinite(Date.parse(a.publishedAt)))));
    newsCheckedAt.value = cache.checkedAt;
  }
} catch { /* A corrupt or unavailable cache should not prevent fresh news. */ }

export async function refreshNews(force = true) {
  if (newsLoading.value || (!force && newsCheckedAt.value && Date.now() - newsCheckedAt.value < REFRESH_INTERVAL)) return;
  newsLoading.value = true;
  newsError.value = null;
  try {
    const articles = parseNewsFeed(await getSpacetimeNews());
    newsArticles.value = articles;
    newsCheckedAt.value = Date.now();
    try { localStorage.setItem(CACHE_KEY, JSON.stringify({ articles, checkedAt: newsCheckedAt.value })); } catch { /* Cache is optional. */ }
  } catch (error) {
    newsError.value = errorMessage(error);
  } finally {
    newsLoading.value = false;
  }
}
