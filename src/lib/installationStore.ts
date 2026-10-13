import { ref } from "vue";
import {
  errorMessage, getLocalInstallations, getSpacetimeReleases, manageSpacetimeVersion,
  type LocalInstallations, type SpacetimeRelease, type VersionAction,
} from "./ecosystem";

// Keep operation state alive when navigating away so a download cannot be
// started a second time and its result is still visible on return.
export const localInstallations = ref<LocalInstallations | null>(null);
export const spacetimeReleases = ref<SpacetimeRelease[]>([]);
export const installationsLoading = ref(false);
export const releasesLoading = ref(false);
export const installationError = ref<string | null>(null);
export const releasesError = ref<string | null>(null);
export const versionOperation = ref<{ action: VersionAction; version?: string } | null>(null);
export const operationError = ref<string | null>(null);
export const operationMessage = ref<string | null>(null);
export const operationOutput = ref("");
export const moreReleasesAvailable = ref(true);
let releasePage = 0;

export async function refreshInstallations() {
  if (installationsLoading.value || versionOperation.value) return;
  installationsLoading.value = true;
  installationError.value = null;
  try { localInstallations.value = await getLocalInstallations(); }
  catch (error) { installationError.value = errorMessage(error); }
  finally { installationsLoading.value = false; }
}

export async function refreshReleases(append = false) {
  if (releasesLoading.value) return;
  releasesLoading.value = true;
  releasesError.value = null;
  try {
    const page = append ? releasePage + 1 : 1;
    const releases = await getSpacetimeReleases(page);
    const existing = append ? spacetimeReleases.value : [];
    spacetimeReleases.value = [...new Map([...existing, ...releases].map((r) => [r.version, r])).values()];
    releasePage = page;
    moreReleasesAvailable.value = releases.length > 0;
  } catch (error) { releasesError.value = errorMessage(error); }
  finally { releasesLoading.value = false; }
}

export async function runVersionOperation(action: VersionAction, version?: string) {
  if (versionOperation.value || installationsLoading.value || !localInstallations.value?.cliPath
    || localInstallations.value.managementError || installationError.value) return;
  versionOperation.value = { action, version };
  operationError.value = null;
  operationMessage.value = null;
  operationOutput.value = "";
  try {
    const result = await manageSpacetimeVersion(action, version);
    localInstallations.value = result.status;
    operationOutput.value = result.output;
    operationMessage.value = action === "install" ? `SpacetimeDB ${version} installed.`
      : action === "uninstall" ? `SpacetimeDB ${version} uninstalled.`
      : `SpacetimeDB ${result.status.currentVersion} is now the active version. Restart your local server to use it.`;
  } catch (error) {
    operationError.value = errorMessage(error);
    // An updater can change files before reporting failure. Always re-detect.
    try { localInstallations.value = await getLocalInstallations(); }
    catch (refreshError) { installationError.value = errorMessage(refreshError); }
  } finally { versionOperation.value = null; }
}
