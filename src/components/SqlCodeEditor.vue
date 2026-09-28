<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import * as monaco from "../../node_modules/monaco-editor/esm/vs/editor/editor.api.js";
import EditorWorker from "../../node_modules/monaco-editor/esm/vs/editor/editor.worker.js?worker";
import "../../node_modules/monaco-editor/esm/vs/editor/contrib/suggest/browser/suggestController.js";
import type { TableSummary } from "../lib/spacetime";
import "../../node_modules/monaco-editor/min/vs/editor/editor.main.css";

const props = defineProps<{
  modelValue: string;
  tables: TableSummary[];
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

Object.assign(globalThis, {
  MonacoEnvironment: {
    getWorker() {
      return new EditorWorker();
    },
  },
});

const host = ref<HTMLDivElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | undefined;
let completionProvider: monaco.IDisposable | undefined;
let resizeObserver: ResizeObserver | undefined;

const sqlKeywords = [
  "SELECT", "FROM", "WHERE", "AND", "OR", "AS", "LIMIT", "JOIN", "INNER", "ON",
  "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "SHOW", "COUNT",
  "TRUE", "FALSE",
];

monaco.languages.register({ id: "sql" });
monaco.languages.setMonarchTokensProvider("sql", {
  ignoreCase: true,
  keywords: sqlKeywords,
  tokenizer: {
    root: [
      [/--.*$/, "comment"],
      [/'(?:[^']|'')*'/, "string"],
      [/"(?:[^"]|"")*"/, "identifier"],
      [/\b(?:0[xX][0-9a-fA-F]*|[0-9]+(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)\b/, "number"],
      [/[a-zA-Z_][\w$]*/, { cases: { "@keywords": "keyword", "@default": "identifier" } }],
      [/[=<>!]+/, "operator"],
      [/[;,.()]/, "delimiter"],
    ],
  },
});

function registerCompletions() {
  completionProvider?.dispose();
  completionProvider = monaco.languages.registerCompletionItemProvider("sql", {
    triggerCharacters: ["."],
    provideCompletionItems(model, position) {
      const word = model.getWordUntilPosition(position);
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      };
      const beforeCursor = model.getValueInRange({
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: position.lineNumber,
        endColumn: position.column,
      });
      const statementStart = beforeCursor.lastIndexOf(";") + 1;
      const cursorOffset = model.getOffsetAt(position);
      const nextSemicolon = model.getValue().indexOf(";", cursorOffset);
      const statement = model.getValue().slice(
        statementStart,
        nextSemicolon < 0 ? model.getValue().length : nextSemicolon,
      );
      const statementPrefix = beforeCursor.slice(statementStart);
      const qualified = statementPrefix.match(/(?:^|[\s,(])([\w$]+)\.([\w$]*)$/);
      const tableExpected = /\b(?:FROM|JOIN|UPDATE|INTO)\s+(?:"[^"]*|[\w$]*)$/i.test(statementPrefix);

      const tablesByQualifier = new Map<string, TableSummary>();
      const referencedTables: TableSummary[] = [];
      for (const match of statement.matchAll(/\b(?:FROM|JOIN|UPDATE|INTO)\s+(?:"([^"]+)"|([\w$]+))(?:\s+(?:AS\s+)?(?:"([^"]+)"|([\w$]+)))?/gi)) {
        const tableName = match[1] ?? match[2];
        const table = props.tables.find((item) => item.name.toLowerCase() === tableName.toLowerCase());
        const alias = match[3] ?? match[4];
        if (table && alias && !/^(WHERE|JOIN|INNER|ON|LIMIT|SET|AND|OR)$/i.test(alias)) {
          tablesByQualifier.set(alias.toLowerCase(), table);
        }
        if (table) {
          tablesByQualifier.set(table.name.toLowerCase(), table);
          if (!referencedTables.includes(table)) referencedTables.push(table);
        }
      }

      const clauses = [...statementPrefix.matchAll(/\b(SELECT|FROM|JOIN|WHERE|AND|OR|ON|SET|UPDATE|INTO|DELETE|INSERT|LIMIT|VALUES)\b/gi)];
      const lastClauseMatch = clauses[clauses.length - 1];
      const lastClause = lastClauseMatch?.[1].toUpperCase() ?? "";
      const clauseTail = lastClauseMatch
        ? statementPrefix.slice(lastClauseMatch.index! + lastClauseMatch[0].length)
        : statementPrefix;
      const tail = clauseTail.trim();
      const trailingSpace = /\s$/.test(statementPrefix);
      const isComparison = /(?:=|!=|<>|<=|>=|<|>)\s*$/.test(tail);
      const hasValue = /(?:=|!=|<>|<=|>=|<|>)\s*(?:true|false|'(?:[^']|'')*'|(?:0[xX][\da-f]+|\d+(?:\.\d+)?(?:[eE][+-]?\d+)?))\s*$/i.test(tail);

      let keywordItems: string[] = [];
      let showTables = false;
      let showFields = false;
      let showComparisons = false;
      let showValues = false;

      if (!statementPrefix.trim()) {
        keywordItems = ["SELECT", "INSERT", "UPDATE", "DELETE", "SHOW", "SET"];
      } else if (tableExpected) {
        showTables = true;
      } else if (qualified) {
        showFields = true;
      } else if (lastClause === "SELECT") {
        if (!tail || tail.endsWith(",") || !trailingSpace) {
          showFields = true;
          if (!tail || tail.endsWith(",")) keywordItems = ["COUNT"];
        } else {
          keywordItems = ["FROM"];
        }
      } else if (lastClause === "FROM" || lastClause === "JOIN") {
        if (tail && !trailingSpace) {
          showTables = true;
        } else {
          keywordItems = lastClause === "JOIN"
            ? ["ON", "JOIN", "WHERE", "LIMIT"]
            : ["JOIN", "WHERE", "LIMIT"];
        }
      } else if (lastClause === "UPDATE") {
        keywordItems = tail && !trailingSpace ? [] : ["SET"];
        showTables = Boolean(tail && !trailingSpace);
      } else if (lastClause === "INTO") {
        showTables = Boolean(tail && !trailingSpace);
        if (trailingSpace && tail) {
          showFields = true;
          keywordItems = ["VALUES"];
        }
      } else if (lastClause === "INSERT") {
        keywordItems = ["INTO"];
      } else if (lastClause === "DELETE") {
        keywordItems = ["FROM"];
      } else if (["WHERE", "AND", "OR", "ON", "SET"].includes(lastClause)) {
        if (hasValue) {
          keywordItems = ["AND", "OR", "LIMIT"];
        } else if (isComparison) {
          showValues = true;
        } else if (tail && trailingSpace) {
          showComparisons = true;
        } else {
          showFields = true;
        }
      } else if (lastClause === "LIMIT" || lastClause === "VALUES") {
        showValues = true;
      } else if (lastClause) {
        keywordItems = sqlKeywords.filter((keyword) =>
          ["SELECT", "INSERT", "UPDATE", "DELETE", "SHOW", "SET"].includes(keyword),
        );
      }

      const suggestions: monaco.languages.CompletionItem[] = keywordItems.map((keyword) => ({
        label: keyword,
        kind: monaco.languages.CompletionItemKind.Keyword,
        insertText: keyword,
        sortText: `0_${keyword}`,
        range,
      }));

      if (showTables) {
        for (const table of props.tables) {
          suggestions.push({
            label: table.name,
            kind: monaco.languages.CompletionItemKind.Struct,
            detail: "SpacetimeDB table",
            insertText: table.name,
            sortText: `1_${table.name}`,
            range,
          });
        }
      }

      const qualifiedTable = qualified
        ? tablesByQualifier.get(qualified[1].toLowerCase())
        : undefined;
      if (showFields) {
        const tables = qualified
          ? (qualifiedTable ? [qualifiedTable] : [])
          : referencedTables.slice(0, 1);
        for (const table of tables) {
          for (const column of table.columns) {
            suggestions.push({
              label: column.name,
              kind: monaco.languages.CompletionItemKind.Field,
              detail: `${table.name} · ${column.type}`,
              insertText: column.name,
              sortText: `2_${table.name}_${column.name}`,
              range,
            });
          }
        }
      }

      if (showComparisons) {
        for (const operator of ["=", "!=", "<>", "<", "<=", ">", ">="]) {
          suggestions.push({
            label: operator,
            kind: monaco.languages.CompletionItemKind.Operator,
            insertText: operator,
            range,
          });
        }
      }

      if (showValues) {
        for (const value of ["TRUE", "FALSE"]) {
          suggestions.push({
            label: value,
            kind: monaco.languages.CompletionItemKind.Keyword,
            insertText: value,
            range,
          });
        }
      }

      return { suggestions };
    },
  });
}

watch(() => props.tables, registerCompletions);
watch(() => props.modelValue, (value) => {
  if (editor && editor.getValue() !== value) editor.setValue(value);
});

onMounted(() => {
  if (!host.value) return;
  editor = monaco.editor.create(host.value, {
    value: props.modelValue,
    language: "sql",
    theme: "vs-dark",
    automaticLayout: true,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    wordWrap: "on",
    tabSize: 2,
    padding: { top: 10, bottom: 10 },
    fontSize: 13,
    lineNumbersMinChars: 3,
    suggestOnTriggerCharacters: true,
    quickSuggestions: { other: true, comments: false, strings: false },
    quickSuggestionsDelay: 0,
    wordBasedSuggestions: "off",
    suggest: { showWords: false, preview: false },
    ariaLabel: "SQL query editor",
  });
  editor.onDidChangeModelContent(() => emit("update:modelValue", editor?.getValue() ?? ""));
  editor.onKeyUp(({ browserEvent }) => {
    const model = editor?.getModel();
    const position = editor?.getPosition();
    const key = browserEvent.key;
    if (!model || !position || key.length !== 1 || !/[\w.$\s]/.test(key)) return;

    const prefix = model.getLineContent(position.lineNumber).slice(0, position.column - 1);
    let inString = false;
    for (let index = 0; index < prefix.length; index += 1) {
      if (!inString && prefix[index] === "-" && prefix[index + 1] === "-") return;
      if (prefix[index] !== "'") continue;
      if (inString && prefix[index + 1] === "'") {
        index += 1;
      } else {
        inString = !inString;
      }
    }
    if (!inString) {
      editor?.trigger("keyboard", "editor.action.triggerSuggest", {});
    }
  });
  registerCompletions();
  resizeObserver = new ResizeObserver(() => editor?.layout());
  resizeObserver.observe(host.value);
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  completionProvider?.dispose();
  editor?.dispose();
});
</script>

<template>
  <div ref="host" class="sql-editor h-full min-h-0 w-full" />
</template>

<style>
.sql-editor .monaco-editor,
.sql-editor .monaco-editor-background,
.sql-editor .monaco-editor .margin {
  background-color: var(--ui-bg) !important;
}
</style>
