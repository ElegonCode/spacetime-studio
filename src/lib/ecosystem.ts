import { invoke } from "@tauri-apps/api/core";

export type InstalledVersion = { version: string; current: boolean };
export type LocalInstallations = {
  cliPath: string | null;
  currentVersion: string | null;
  versions: InstalledVersion[];
  managementError: string | null;
};
export type SpacetimeRelease = {
  version: string;
  url: string;
  publishedAt: string | null;
  prerelease: boolean;
};
export type VersionAction = "install" | "use" | "upgrade" | "uninstall";
export type VersionActionResult = { output: string; status: LocalInstallations };

export const getSpacetimeNews = () => invoke<string>("get_spacetime_news");
export const getLocalInstallations = () => invoke<LocalInstallations>("get_local_installations");
export const getSpacetimeReleases = (page = 1) => invoke<SpacetimeRelease[]>("get_spacetime_releases", { page });
export const manageSpacetimeVersion = (action: VersionAction, version?: string) =>
  invoke<VersionActionResult>("manage_spacetime_version", { action, version: version ?? null });

export function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
