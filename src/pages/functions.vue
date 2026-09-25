<script setup lang="ts">
import type { TableColumn, TableRow } from "@nuxt/ui";
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  getSchema,
  getSelectedConnectionId,
  parseCell,
  runFunction,
  type ColumnSummary,
  type FunctionSummary,
  type SchemaSummary,
} from "../lib/spacetime";
import {
  clearPageRefreshHandler,
  setPageRefreshHandler,
} from "../lib/pageActions";
import { dataTableUi } from "../lib/tableUi";

type FunctionParamValue = string | boolean;
type BadgeColor = "info" | "success" | "warning" | "neutral";

const connectionId = ref<string | null>(getSelectedConnectionId());
const schema = ref<SchemaSummary | null>(null);
const loading = ref(false);
const running = ref(false);
const error = ref("");
const status = ref("");
const filter = ref("");
const runnerOpen = ref(false);
const selectedFunctionName = ref("");
const paramForm = ref<Record<string, FunctionParamValue>>({});
const runResult = ref<unknown>(null);

const functions = computed<FunctionSummary[]>(() => {
  const query = filter.value.trim().toLowerCase();
  const reducers = schema.value?.reducers ?? [];
  if (!query) return reducers;

  return reducers.filter((item) => item.name.toLowerCase().includes(query));
});

// Parameters drops its right border because the pinned Actions column already
// draws one on its left.
const columns: TableColumn<FunctionSummary>[] = [
  {
    id: "name",
    header: "Function",
    meta: {
      class: {
        th: "whitespace-nowrap border-r border-default",
        td: "min-w-56 border-r border-default/60 align-top",
      },
    },
  },
  {
    id: "type",
    header: "Type",
    meta: {
      class: {
        th: "whitespace-nowrap border-r border-default",
        td: "whitespace-nowrap border-r border-default/60 align-top",
      },
    },
  },
  {
    id: "params",
    header: "Parameters",
    meta: { class: { th: "min-w-80", td: "align-top" } },
  },
  {
    id: "actions",
    header: "Actions",
    meta: {
      class: {
        th: "w-28 text-right",
        td: "whitespace-nowrap text-right align-top",
      },
    },
  },
];

function getFunctionRowId(fn: FunctionSummary) {
  return fn.name;
}

function onSelectFunction(_: Event, row: TableRow<FunctionSummary>) {
  openRunner(row.original);
}

const selectedFunction = computed<FunctionSummary | null>(
  () =>
    (schema.value?.reducers ?? []).find(
      (fn) => fn.name === selectedFunctionName.value,
    ) ?? null,
);

const formattedRunResult = computed(() => {
  if (runResult.value === null || runResult.value === undefined) return "";
  return JSON.stringify(runResult.value, null, 2);
});

function normalizeType(type: string) {
  return type.toLowerCase().replace(/[^a-z0-9]/g, "");
}

function isBooleanParam(param: ColumnSummary) {
  return ["bool", "boolean"].includes(normalizeType(param.type));
}

function isNumberParam(param: ColumnSummary) {
  return (
    /^(u|i)(8|16|32|64|128)$/.test(normalizeType(param.type)) ||
    /^(f)(32|64)$/.test(normalizeType(param.type)) ||
    ["usize", "isize"].includes(normalizeType(param.type))
  );
}

function isStringParam(param: ColumnSummary) {
  return ["str", "string"].includes(normalizeType(param.type));
}

function paramBadgeColor(param: ColumnSummary): BadgeColor {
  if (isStringParam(param)) return "info";
  if (isNumberParam(param)) return "success";
  if (isBooleanParam(param)) return "warning";
  return "neutral";
}

function defaultParamValue(param: ColumnSummary): FunctionParamValue {
  return isBooleanParam(param) ? false : "";
}

function openRunner(fn: FunctionSummary) {
  selectedFunctionName.value = fn.name;
  paramForm.value = Object.fromEntries(
    fn.params.map((param) => [param.name, defaultParamValue(param)]),
  );
  runResult.value = null;
  status.value = "";
  error.value = "";
  runnerOpen.value = true;
}

function getTextParam(paramName: string) {
  return String(paramForm.value[paramName] ?? "");
}

function setTextParam(paramName: string, value: string | number) {
  paramForm.value[paramName] = String(value);
}

function getBooleanParam(paramName: string) {
  return paramForm.value[paramName] === true;
}

function setBooleanParam(
  paramName: string,
  value: boolean | "indeterminate",
) {
  paramForm.value[paramName] = value === true;
}

function paramToArg(param: ColumnSummary) {
  const value = paramForm.value[param.name];
  if (isBooleanParam(param)) return value === true;

  const text = String(value ?? "");
  if (isStringParam(param)) return text;
  return parseCell(text);
}

async function load() {
  connectionId.value = getSelectedConnectionId();
  if (!connectionId.value) {
    error.value = "Select or create a connection first.";
    return;
  }

  loading.value = true;
  error.value = "";
  status.value = "";

  try {
    schema.value = await getSchema(connectionId.value);
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

async function runSelectedFunction() {
  if (!connectionId.value || !selectedFunction.value) return;

  const args = selectedFunction.value.params.map(paramToArg);
  running.value = true;
  error.value = "";
  status.value = "";
  runResult.value = null;

  try {
    const result = await runFunction(
      connectionId.value,
      selectedFunction.value.name,
      args,
    );
    runResult.value = result.results;
    status.value = `${selectedFunction.value.name} ran successfully.`;
  } catch (err) {
    error.value = String(err);
  } finally {
    running.value = false;
  }
}

onMounted(() => {
  load();
  setPageRefreshHandler(load);
});

onUnmounted(() => {
  clearPageRefreshHandler(load);
});
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-4 p-4">
    <div class="shrink-0 flex flex-wrap items-center justify-between gap-3">
      <UInput
        v-model="filter"
        icon="i-lucide-search"
        placeholder="Filter functions"
        aria-label="Filter functions"
        class="w-full sm:max-w-md"
      />
      <UBadge color="neutral" variant="subtle"
        >{{ functions.length }}/{{ schema?.reducers.length ?? 0 }} functions</UBadge
      >
    </div>

    <UAlert
      v-if="error"
      color="error"
      variant="subtle"
      :description="error"
      class="shrink-0"
    />
    <UTable
      :data="functions"
      :columns="columns"
      :get-row-id="getFunctionRowId"
      :meta="{ class: { tr: 'cursor-pointer transition' } }"
      sticky="header"
      :column-pinning="{ right: ['actions'] }"
      class="min-h-0 flex-1 rounded-md border border-default bg-default/30"
      :ui="{ ...dataTableUi, empty: loading ? 'hidden' : dataTableUi.empty }"
      @select="onSelectFunction"
    >
      <template #name-cell="{ row }">
        <span class="font-medium text-highlighted">{{ row.original.name }}</span>
      </template>

      <template #type-cell="{ row }">
        <UBadge color="neutral" variant="subtle">{{
          row.original.lifecycle ?? "callable"
        }}</UBadge>
      </template>

      <template #params-cell="{ row }">
        <div v-if="row.original.params.length" class="flex flex-wrap gap-2">
          <UBadge
            v-for="param in row.original.params"
            :key="param.name"
            variant="soft"
            :color="paramBadgeColor(param)"
          >
            {{ param.name }}: {{ param.type }}
          </UBadge>
        </div>
        <span v-else class="text-muted">No parameters</span>
      </template>

      <template #actions-cell="{ row }">
        <UButton
          icon="i-lucide-play"
          size="xs"
          variant="ghost"
          aria-label="Run function"
          @click.stop="openRunner(row.original)"
        />
      </template>

      <template #empty>No functions found.</template>
    </UTable>

    <USlideover
      v-model:open="runnerOpen"
      title="Run Function"
      :description="selectedFunctionName"
      side="right"
      :ui="{
        content: 'sm:max-w-xl',
        body: 'min-h-0 flex-1',
        footer: 'shrink-0',
      }"
    >
      <template #body>
        <div class="space-y-5">
          <UAlert
            v-if="status"
            color="success"
            variant="subtle"
            :description="status"
          />

          <UForm
            id="function-runner-form"
            :state="paramForm"
            class="space-y-4"
            @submit="runSelectedFunction"
          >
            <p
              v-if="!selectedFunction?.params.length"
              class="rounded-md border border-default bg-default/30 px-3 py-2 text-sm text-muted"
            >
              This function does not need parameters.
            </p>

            <UFormField
              v-for="param in selectedFunction?.params ?? []"
              :key="param.name"
              :label="param.name"
              :hint="param.type"
            >
              <UCheckbox
                v-if="isBooleanParam(param)"
                :model-value="getBooleanParam(param.name)"
                @update:model-value="setBooleanParam(param.name, $event)"
              />
              <UInput
                v-else-if="isNumberParam(param)"
                :model-value="getTextParam(param.name)"
                class="w-full"
                type="number"
                @update:model-value="setTextParam(param.name, $event)"
              />
              <UInput
                v-else-if="isStringParam(param)"
                :model-value="getTextParam(param.name)"
                class="w-full"
                @update:model-value="setTextParam(param.name, $event)"
              />
              <UTextarea
                v-else
                :model-value="getTextParam(param.name)"
                class="w-full"
                :rows="4"
                placeholder="JSON value"
                @update:model-value="setTextParam(param.name, $event)"
              />
            </UFormField>
          </UForm>

          <div v-if="formattedRunResult" class="space-y-2">
            <p class="text-xs font-medium text-muted">Result</p>
            <pre
              class="max-h-72 overflow-auto rounded-md border border-default bg-default/40 p-3 text-xs text-highlighted"
            >{{ formattedRunResult }}</pre>
          </div>
        </div>
      </template>

      <template #footer="{ close }">
        <div class="flex w-full justify-end gap-2">
          <UButton color="neutral" variant="soft" @click="close"
            >Cancel</UButton
          >
          <UButton
            icon="i-lucide-play"
            :loading="running"
            type="submit"
            form="function-runner-form"
          >
            Run
          </UButton>
        </div>
      </template>
    </USlideover>
  </div>
</template>
