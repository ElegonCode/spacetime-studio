export const MAINCLOUD_URL = "https://maincloud.spacetimedb.com";
export const LOCALHOST_URL = "http://localhost:3000";

export type HostMode = "local" | "maincloud" | "custom";

export function hostModeForUrl(baseUrl: string): HostMode {
  if (baseUrl === MAINCLOUD_URL) return "maincloud";
  if (baseUrl === LOCALHOST_URL) return "local";
  return "custom";
}

export function hostLabel(baseUrl: string) {
  if (baseUrl === MAINCLOUD_URL) return "Maincloud";
  return baseUrl.replace(/^https?:\/\//, "");
}
