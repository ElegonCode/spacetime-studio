<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { errorMessage } from "../lib/ecosystem";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
  type PageRefreshReason,
} from "../lib/pageActions";
import {
  localInstallations,
  spacetimeReleases,
  installationsLoading,
  releasesLoading,
  installationError,
  releasesError,
  versionOperation,
  operationError,
  operationMessage,
  operationOutput,
  moreReleasesAvailable,
  refreshInstallations,
  refreshReleases,
  runVersionOperation,
} from "../lib/installationStore";

const search = ref("");
const showPrereleases = ref(false);
const linkError = ref<string | null>(null);
const pendingUninstall = ref<string | null>(null);
const installed = computed(
  () => new Set(localInstallations.value?.versions.map((v) => v.version) ?? []),
);
const latestStable = computed(() =>
  spacetimeReleases.value.find((v) => !v.prerelease && !v.version.includes("-")),
);
const canManage = computed(
  () =>
    Boolean(localInstallations.value?.cliPath) &&
    !localInstallations.value?.managementError &&
    !installationError.value &&
    !installationsLoading.value &&
    !versionOperation.value,
);
const availableVersions = computed(() =>
  spacetimeReleases.value.filter(
    (v) =>
      (showPrereleases.value || !v.prerelease) &&
      v.version.toLowerCase().includes(search.value.trim().toLowerCase()),
  ),
);
const updateAvailable = computed(() => {
  const current = localInstallations.value?.currentVersion;
  const latest = latestStable.value?.version;
  if (!latest) return false;
  if (!current) return true;
  const currentNumbers = current.split("-")[0]!.split(".").map(Number);
  const latestNumbers = latest.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if (latestNumbers[i] !== currentNumbers[i]) return latestNumbers[i]! > currentNumbers[i]!;
  }
  return current.includes("-");
});
const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });
const operationTitle = computed(() => {
  const operation = versionOperation.value;
  if (!operation) return "";
  switch (operation.action) {
    case "upgrade":
      return "Updating SpacetimeDB…";
    case "install":
      return `Installing SpacetimeDB ${operation.version}…`;
    case "use":
      return `Switching to SpacetimeDB ${operation.version}…`;
    case "uninstall":
      return `Uninstalling SpacetimeDB ${operation.version}…`;
  }
});
async function confirmUninstall() {
  const version = pendingUninstall.value;
  if (!version || !canManage.value || version === localInstallations.value?.currentVersion) return;
  pendingUninstall.value = null;
  await runVersionOperation("uninstall", version);
}
function onUninstallModalOpenChange(open: boolean) {
  if (!open) pendingUninstall.value = null;
}
const refresh = async (reason?: PageRefreshReason) => {
  if (reason === "connection-change" || versionOperation.value) return;
  await Promise.all([refreshInstallations(), refreshReleases()]);
};
async function openLink(url: string) {
  linkError.value = null;
  try {
    await openUrl(url);
  } catch (error) {
    linkError.value = errorMessage(error);
  }
}
onMounted(() => {
  setPageRefreshHandler(refresh);
  void refreshInstallations();
  if (!spacetimeReleases.value.length) void refreshReleases();
});
onUnmounted(() => clearPageRefreshHandler(refresh));
</script>

<template>
  <div class="h-full overflow-y-auto p-6">
    <div class="mx-auto max-w-5xl space-y-6">
      <div>
        <h1 class="text-lg font-semibold text-highlighted">Local SpacetimeDB installations</h1>
        <p class="mt-1 text-sm text-muted">
          Install or uninstall versions, switch the active version, or update to the latest release.
          Start and stop the local server from the connection picker in the sidebar.
        </p>
      </div>

      <UAlert
        v-if="installationError"
        color="error"
        variant="subtle"
        title="Could not check the local installation"
        :description="installationError"
      >
        <template #actions
          ><UButton
            size="xs"
            color="neutral"
            variant="soft"
            :loading="installationsLoading"
            :disabled="Boolean(versionOperation)"
            @click="refreshInstallations"
            >Retry</UButton
          ></template
        >
      </UAlert>
      <UAlert
        v-if="operationError"
        color="error"
        variant="subtle"
        icon="i-lucide-circle-alert"
        title="Version change failed"
        :description="operationError"
        :ui="{ description: 'whitespace-pre-wrap break-words' }"
      />
      <UAlert
        v-if="operationMessage"
        color="success"
        variant="subtle"
        icon="i-lucide-check"
        :title="operationMessage"
      />
      <UAlert
        v-if="versionOperation"
        color="info"
        variant="subtle"
        icon="i-lucide-loader-circle"
        :title="operationTitle"
        description="This may take a few minutes. You can use the other pages while it finishes."
        role="status"
      />
      <UAlert v-if="linkError" color="error" title="Could not open link" :description="linkError" />

      <UCard>
        <div
          v-if="installationsLoading && !localInstallations"
          role="status"
          aria-label="Checking SpacetimeDB installation"
          class="space-y-3"
        >
          <USkeleton class="h-4 w-32" /><USkeleton class="h-9 w-44" /><USkeleton
            class="h-4 w-3/4"
          />
        </div>
        <template v-else-if="localInstallations?.cliPath">
          <div class="flex flex-wrap items-start justify-between gap-4">
            <div>
              <p class="text-xs font-medium uppercase tracking-wide text-muted">Active version</p>
              <div class="mt-2 flex flex-wrap items-center gap-3">
                <p class="text-2xl font-semibold text-highlighted">
                  {{ localInstallations.currentVersion ?? "No version selected" }}
                </p>
                <UBadge
                  v-if="latestStable && !updateAvailable && localInstallations.currentVersion"
                  color="success"
                  variant="subtle"
                  >Up to date</UBadge
                >
                <UBadge v-else-if="latestStable" color="primary" variant="subtle"
                  >{{ latestStable.version }} available</UBadge
                >
              </div>
            </div>
            <UButton
              v-if="updateAvailable || versionOperation?.action === 'upgrade'"
              icon="i-lucide-download"
              :disabled="!canManage || !updateAvailable"
              :loading="versionOperation?.action === 'upgrade'"
              @click="runVersionOperation('upgrade')"
              >Update to latest</UButton
            >
          </div>
          <p class="mt-4 break-all font-mono text-xs text-muted">
            {{ localInstallations.cliPath }}
          </p>
          <p class="mt-3 text-sm text-muted">
            The active version is used by the CLI and new local server launches. Restart an already
            running server after switching versions.
          </p>
          <UAlert
            v-if="localInstallations.managementError"
            class="mt-4"
            color="warning"
            variant="subtle"
            title="The local version manager is unavailable"
            :description="localInstallations.managementError"
            :ui="{ description: 'whitespace-pre-wrap break-words' }"
          >
            <template #actions
              ><UButton
                size="xs"
                color="neutral"
                variant="soft"
                @click="openLink('https://spacetimedb.com/install')"
                >Installation guide</UButton
              ></template
            >
          </UAlert>
        </template>
        <div
          v-else-if="!installationError"
          class="flex flex-wrap items-start justify-between gap-4"
        >
          <div>
            <h2 class="font-medium text-highlighted">SpacetimeDB is not installed</h2>
            <p class="mt-1 max-w-xl text-sm text-muted">
              Install the official SpacetimeDB CLI, then refresh this page to manage its versions.
            </p>
          </div>
          <UButton
            icon="i-lucide-external-link"
            color="neutral"
            variant="soft"
            @click="openLink('https://spacetimedb.com/install')"
            >Install SpacetimeDB</UButton
          >
        </div>
      </UCard>

      <section v-if="localInstallations?.versions.length" class="space-y-3">
        <h2 class="font-medium text-highlighted">
          Installed versions
          <span class="ml-1 text-sm font-normal text-muted">{{
            localInstallations.versions.length
          }}</span>
        </h2>
        <p class="text-xs text-muted">
          Switch to another version before uninstalling the active version.
        </p>
        <div class="divide-y divide-default rounded-lg border border-default">
          <div
            v-for="version in localInstallations.versions"
            :key="version.version"
            class="flex flex-wrap items-center justify-between gap-3 px-4 py-3"
          >
            <div class="flex items-center gap-3">
              <span class="font-mono text-sm text-highlighted">{{ version.version }}</span
              ><UBadge v-if="version.current" color="primary" variant="subtle">Active</UBadge>
            </div>
            <div class="flex items-center gap-2">
              <UButton
                v-if="!version.current"
                size="sm"
                color="neutral"
                variant="soft"
                icon="i-lucide-check"
                :disabled="!canManage"
                :loading="
                  versionOperation?.action === 'use' && versionOperation.version === version.version
                "
                @click="runVersionOperation('use', version.version)"
                >Use version</UButton
              >
              <UButton
                size="sm"
                color="error"
                variant="ghost"
                icon="i-lucide-trash-2"
                :disabled="!canManage || version.current"
                :loading="
                  versionOperation?.action === 'uninstall' &&
                  versionOperation.version === version.version
                "
                :aria-label="`Uninstall SpacetimeDB ${version.version}`"
                :title="
                  version.current
                    ? 'Switch to another version before uninstalling this version'
                    : `Uninstall SpacetimeDB ${version.version}`
                "
                @click="pendingUninstall = version.version"
                >Uninstall</UButton
              >
            </div>
          </div>
        </div>
      </section>

      <section class="space-y-4">
        <div class="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 class="font-medium text-highlighted">Available versions</h2>
            <p class="mt-1 text-xs text-muted">
              Official releases with binaries for this computer.
            </p>
          </div>
          <div class="flex flex-wrap items-center gap-4">
            <UCheckbox v-model="showPrereleases" label="Include prereleases" />
            <UInput
              v-model="search"
              icon="i-lucide-search"
              placeholder="Find a version…"
              aria-label="Find a SpacetimeDB version"
            />
          </div>
        </div>
        <UAlert
          v-if="releasesError"
          color="error"
          variant="subtle"
          title="Could not load available versions"
          :description="releasesError"
        >
          <template #actions
            ><UButton
              size="xs"
              color="neutral"
              variant="soft"
              :loading="releasesLoading"
              @click="refreshReleases()"
              >Retry</UButton
            ></template
          >
        </UAlert>
        <div
          v-if="releasesLoading && !spacetimeReleases.length"
          role="status"
          aria-label="Loading available versions"
          class="space-y-2"
        >
          <USkeleton v-for="n in 5" :key="n" class="h-16 w-full" />
        </div>
        <div
          v-else-if="availableVersions.length"
          class="divide-y divide-default rounded-lg border border-default"
        >
          <div
            v-for="release in availableVersions"
            :key="release.version"
            class="flex flex-wrap items-center justify-between gap-3 px-4 py-3"
          >
            <div>
              <div class="flex flex-wrap items-center gap-2">
                <span class="font-mono text-sm text-highlighted">{{ release.version }}</span>
                <UBadge
                  v-if="release.version === latestStable?.version"
                  color="success"
                  variant="subtle"
                  size="sm"
                  >Latest stable</UBadge
                >
                <UBadge v-if="release.prerelease" color="warning" variant="subtle" size="sm"
                  >Prerelease</UBadge
                >
                <UBadge
                  v-if="installed.has(release.version)"
                  color="neutral"
                  variant="subtle"
                  size="sm"
                  >Installed</UBadge
                >
              </div>
              <p v-if="release.publishedAt" class="mt-1 text-xs text-muted">
                {{ dateFormat.format(new Date(release.publishedAt)) }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <UButton
                size="sm"
                icon="i-lucide-external-link"
                color="neutral"
                variant="ghost"
                :aria-label="`Release notes for ${release.version}`"
                title="Release notes"
                @click="openLink(release.url)"
              />
              <UButton
                v-if="!installed.has(release.version)"
                size="sm"
                icon="i-lucide-download"
                color="neutral"
                variant="soft"
                :disabled="!canManage"
                :loading="
                  versionOperation?.action === 'install' &&
                  versionOperation.version === release.version
                "
                @click="runVersionOperation('install', release.version)"
                >Install</UButton
              >
              <UButton
                v-else-if="release.version !== localInstallations?.currentVersion"
                size="sm"
                icon="i-lucide-check"
                color="neutral"
                variant="soft"
                :disabled="!canManage"
                :loading="
                  versionOperation?.action === 'use' && versionOperation.version === release.version
                "
                @click="runVersionOperation('use', release.version)"
                >Use version</UButton
              >
              <span v-else class="px-3 text-xs text-muted">Active</span>
            </div>
          </div>
        </div>
        <p
          v-else-if="!releasesError"
          class="rounded-lg border border-default p-8 text-center text-sm text-muted"
        >
          No versions match your filters.
        </p>
        <UButton
          v-if="spacetimeReleases.length && moreReleasesAvailable"
          color="neutral"
          variant="soft"
          :loading="releasesLoading"
          @click="refreshReleases(true)"
          >Load older releases</UButton
        >
      </section>

      <details v-if="operationOutput" class="rounded-lg border border-default p-4">
        <summary class="cursor-pointer text-sm text-muted">Version manager output</summary>
        <pre
          class="mt-3 max-h-64 overflow-auto whitespace-pre-wrap break-words text-xs text-muted"
          >{{ operationOutput }}</pre>
      </details>
    </div>
    <UModal
      :open="pendingUninstall !== null"
      title="Uninstall SpacetimeDB version"
      :description="`Remove SpacetimeDB ${pendingUninstall ?? ''} from this computer? You can install it again later. Stop any local server using this version before uninstalling it.`"
      @update:open="onUninstallModalOpenChange"
    >
      <template #footer>
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="pendingUninstall = null">Cancel</UButton>
          <UButton
            color="error"
            icon="i-lucide-trash-2"
            :disabled="
              !canManage ||
              !pendingUninstall ||
              pendingUninstall === localInstallations?.currentVersion ||
              !installed.has(pendingUninstall)
            "
            @click="confirmUninstall"
            >Uninstall</UButton
          >
        </div>
      </template>
    </UModal>
  </div>
</template>
