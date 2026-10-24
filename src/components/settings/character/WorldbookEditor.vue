<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import {
  previewCharacterWorldbook,
  type Worldbook,
  type WorldbookEntry,
  type WorldbookKey,
  type WorldbookIssue,
  type WorldbookMatch,
} from "@/api/services/character";

const book = defineModel<Worldbook | null>({ required: true });
const props = defineProps<{ baseline: Worldbook | null; loadError: string }>();
const { t } = useI18n();
const dialog = useDialogStore();
const selected = ref(0);
const query = ref("");
const input = ref("");
const issues = ref<WorldbookIssue[]>([]);
const results = ref<WorldbookMatch[] | null>(null);
const error = ref("");
const testing = ref(false);
const cachedKeys = new Map<string, WorldbookKey[]>();
let sequence = 0;
const field =
  "w-full min-w-0 rounded-lg border border-white/15 bg-black/20 px-3 py-2 text-sm text-white outline-none focus:border-cyan-400/60";
const button =
  "rounded-lg border border-white/15 px-3 py-2 text-xs text-white/80 transition hover:bg-white/10 disabled:opacity-30";
const current = computed(() => book.value?.entries[selected.value]);
const ordered = computed(() =>
  (book.value?.entries ?? [])
    .map((entry, index) => ({ entry, index }))
    .sort(
      (a, b) =>
        Number(b.entry.activation.type === "always") -
          Number(a.entry.activation.type === "always") || b.entry.priority - a.entry.priority,
    ),
);
const visible = computed(() =>
  ordered.value.filter(({ entry }) => {
    const keys =
      entry.activation.type === "keyword" ? entry.activation.keys.map(keyText).join(" ") : "";
    return `${entry.name ?? ""} ${entry.id} ${keys} ${entry.content}`
      .toLowerCase()
      .includes(query.value.toLowerCase());
  }),
);
const summary = computed(() => {
  const before = props.baseline?.entries ?? [];
  const after = book.value?.entries ?? [];
  return {
    added: after.filter((entry) => !before.some((old) => old.id === entry.id)).length,
    removed: before.filter((entry) => !after.some((next) => next.id === entry.id)).length,
    modified: after.filter((entry) =>
      before.some((old) => old.id === entry.id && JSON.stringify(old) !== JSON.stringify(entry)),
    ).length,
  };
});
const changed = computed(() => JSON.stringify(book.value) !== JSON.stringify(props.baseline));
const entryIssues = computed(() =>
  issues.value.filter((issue) => issue.entry_index === selected.value),
);

function matchedKeyLabels(result: WorldbookMatch) {
  const activation = book.value?.entries[result.entry_index]?.activation;
  if (activation?.type !== "keyword") return "";
  return result.matched_keys
    .map((index) => `${index + 1}: ${keyText(activation.keys[index]!)}`)
    .join("、");
}
function keyText(key: WorldbookKey) {
  return typeof key === "string" ? key : key.pattern;
}
function addEntry(source?: WorldbookEntry) {
  if (!book.value || book.value.entries.length >= 500) return;
  const entry: WorldbookEntry = source
    ? JSON.parse(JSON.stringify(source))
    : {
        id: "",
        name: t("settings.worldbook.newEntry"),
        enabled: true,
        priority: 0,
        activation: { type: "keyword", keys: [""] },
        content: "",
      };
  entry.id = crypto.randomUUID();
  book.value.entries.push(entry);
  selected.value = book.value.entries.length - 1;
  query.value = "";
}
async function removeEntry() {
  if (!book.value || !current.value) return;
  if (!(await dialog.confirm(t("settings.worldbook.confirmEntry")))) return;
  cachedKeys.delete(current.value.id);
  book.value.entries.splice(selected.value, 1);
  selected.value = Math.max(0, selected.value - 1);
}
async function removeBook() {
  if (await dialog.confirm(t("settings.worldbook.confirmBook"))) book.value = null;
}
function setActivation(type: string) {
  const entry = current.value;
  if (!entry) return;
  if (entry.activation.type === "keyword") cachedKeys.set(entry.id, entry.activation.keys);
  entry.activation =
    type === "always"
      ? { type: "always" }
      : { type: "keyword", keys: cachedKeys.get(entry.id) ?? [""] };
}
function setKeyType(index: number, type: string) {
  const activation = current.value?.activation;
  if (activation?.type !== "keyword") return;
  const key = activation.keys[index]!;
  if (type === "regex" && typeof key === "string") {
    activation.keys[index] = {
      type: "regex",
      pattern: key.trim().replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
      case_sensitive: false,
    };
  } else if (type === "literal" && typeof key !== "string") {
    activation.keys[index] = key.pattern;
  }
}
function moveTarget(offset: number) {
  const position = visible.value.findIndex((item) => item.index === selected.value);
  const neighbor = position < 0 ? undefined : visible.value[position + offset];
  return neighbor?.entry.activation.type === current.value?.activation.type ? neighbor : undefined;
}
function move(offset: number) {
  if (!book.value || !current.value) return;
  const neighbor = moveTarget(offset);
  if (!neighbor) return;
  const source = current.value;
  const next = ordered.value.map(({ entry }) => entry);
  const from = next.indexOf(source);
  const to = next.indexOf(neighbor.entry);
  next.splice(from, 1);
  next.splice(to, 0, source);
  const group = next.filter((entry) => entry.activation.type === source.activation.type);
  group.forEach((entry, index) => {
    entry.priority = group.length - index - 1;
  });
  book.value.entries = next;
  selected.value = next.indexOf(source);
}
function canMove(offset: number) {
  return !!moveTarget(offset);
}
async function check(showResults = false): Promise<boolean> {
  if (props.loadError) return false;
  if (!book.value) return true;
  const invalidPriorities = book.value.entries.flatMap((entry, index) =>
    !Number.isInteger(entry.priority) || entry.priority < -2147483648 || entry.priority > 2147483647
      ? [{ entry_index: index, key_index: null, message: t("settings.worldbook.invalidPriority") }]
      : [],
  );
  if (invalidPriorities.length) {
    issues.value = invalidPriorities;
    selected.value = invalidPriorities[0]!.entry_index;
    results.value = null;
    return false;
  }
  const request = ++sequence;
  testing.value = true;
  error.value = "";
  try {
    const preview = await previewCharacterWorldbook(book.value, input.value);
    if (request !== sequence) return false;
    issues.value = preview.issues;
    if (showResults) results.value = preview.entries;
    const first = preview.issues[0]?.entry_index;
    if (first != null && !showResults) selected.value = first;
    return !preview.issues.length;
  } catch (cause) {
    if (request === sequence) error.value = String(cause);
    return false;
  } finally {
    if (request === sequence) testing.value = false;
  }
}
watch(
  book,
  () => {
    ++sequence;
    testing.value = false;
    issues.value = [];
    results.value = null;
    error.value = "";
  },
  { deep: true, flush: "sync" },
);
watch(input, () => {
  ++sequence;
  testing.value = false;
  results.value = null;
});
defineExpose({ validate: () => check() });
</script>

<template>
  <section class="space-y-4 text-sm text-white/80">
    <p class="text-xs leading-relaxed text-white/50">{{ t("settings.worldbook.hint") }}</p>
    <p v-if="loadError" role="alert" class="rounded-lg bg-red-400/10 p-3 text-red-200">
      {{ loadError }}<br />{{ t("settings.worldbook.readError") }}
    </p>
    <template v-else-if="!book">
      <p>{{ t("settings.worldbook.empty") }}</p>
      <button
        type="button"
        :class="button"
        @click="
          book = { name: '', entries: [] };
          selected = 0;
        "
      >
        {{ t("settings.worldbook.create") }}
      </button>
      <p v-if="changed" class="text-amber-200">{{ t("settings.worldbook.pendingRemoval") }}</p>
    </template>
    <template v-else>
      <div class="flex flex-wrap items-end gap-2">
        <label class="min-w-40 flex-1 space-y-1"
          >{{ t("settings.worldbook.bookName") }}
          <input v-model="book.name" :class="field" />
        </label>
        <button type="button" :class="button" @click="removeBook">
          {{ t("settings.worldbook.removeBook") }}
        </button>
      </div>
      <p class="text-xs text-amber-100/65">{{ t("settings.worldbook.rewriteHint") }}</p>
      <p v-if="changed" class="text-xs text-cyan-200">
        {{ t("settings.worldbook.summary", summary) }}
      </p>
      <p class="text-xs text-white/50">{{ t("settings.worldbook.orderHint") }}</p>
      <div class="grid gap-4 xl:grid-cols-[12rem_minmax(0,1fr)]">
        <aside class="space-y-2">
          <input
            v-model="query"
            :class="field"
            :aria-label="t('settings.worldbook.search')"
            :placeholder="t('settings.worldbook.search')"
          />
          <button
            type="button"
            :class="button"
            :disabled="book.entries.length >= 500"
            @click="addEntry()"
          >
            {{ t("settings.worldbook.add") }} ({{ book.entries.length }}/500)
          </button>
          <div class="max-h-80 space-y-1 overflow-y-auto">
            <template v-for="({ entry, index }, position) in visible" :key="index">
              <p
                v-if="
                  position === 0 ||
                  visible[position - 1]?.entry.activation.type !== entry.activation.type
                "
                class="pt-2 pb-1 text-xs font-semibold text-white/60"
              >
                {{ t(`settings.worldbook.${entry.activation.type}`) }}
              </p>
              <button
                type="button"
                class="block w-full rounded-lg border p-2 text-left"
                :class="
                  index === selected ? 'border-cyan-400/50 bg-cyan-400/10' : 'border-white/10'
                "
                @click="selected = index"
              >
                <span class="block truncate" :class="{ 'opacity-40': !entry.enabled }">{{
                  entry.name || entry.id
                }}</span>
                <span class="text-xs text-white/45"
                  >{{ t(`settings.worldbook.${entry.activation.type}`) }} ·
                  {{ entry.priority }}</span
                >
                <span
                  v-if="issues.some((issue) => issue.entry_index === index)"
                  class="ml-1 text-red-300"
                  >!</span
                >
              </button>
            </template>
          </div>
        </aside>
        <div v-if="current" class="min-w-0 space-y-3 rounded-xl border border-white/10 p-3">
          <div class="flex flex-wrap gap-2">
            <button type="button" :class="button" @click="addEntry(current)">
              {{ t("settings.worldbook.copy") }}
            </button>
            <button type="button" :class="button" :disabled="!canMove(-1)" @click="move(-1)">
              {{ t("settings.worldbook.up") }}
            </button>
            <button type="button" :class="button" :disabled="!canMove(1)" @click="move(1)">
              {{ t("settings.worldbook.down") }}
            </button>
            <button type="button" :class="button" @click="removeEntry">
              {{ t("settings.worldbook.removeEntry") }}
            </button>
          </div>
          <label class="block space-y-1"
            >{{ t("settings.worldbook.entryName") }}<input v-model="current.name" :class="field"
          /></label>
          <label class="flex items-center gap-2"
            ><input v-model="current.enabled" type="checkbox" />{{
              t("settings.worldbook.enabled")
            }}</label
          >
          <div class="grid grid-cols-2 gap-3">
            <label class="space-y-1"
              >{{ t("settings.worldbook.priority")
              }}<input
                v-model.number="current.priority"
                type="number"
                step="1"
                min="-2147483648"
                max="2147483647"
                :class="field"
            /></label>
            <label class="space-y-1"
              >{{ t("settings.worldbook.activation") }}
              <select
                :value="current.activation.type"
                :class="field"
                @change="setActivation(($event.target as HTMLSelectElement).value)"
              >
                <option value="always">{{ t("settings.worldbook.always") }}</option>
                <option value="keyword">{{ t("settings.worldbook.keyword") }}</option>
              </select>
            </label>
          </div>
          <div v-if="current.activation.type === 'keyword'" class="space-y-2">
            <div
              v-for="(key, index) in current.activation.keys"
              :key="index"
              class="space-y-1 rounded-lg bg-white/5 p-2"
            >
              <div class="flex flex-wrap gap-2">
                <select
                  :aria-label="t('settings.worldbook.keyType')"
                  :value="typeof key === 'string' ? 'literal' : 'regex'"
                  :class="[field, 'w-auto!']"
                  @change="setKeyType(index, ($event.target as HTMLSelectElement).value)"
                >
                  <option value="literal">{{ t("settings.worldbook.literal") }}</option>
                  <option value="regex">{{ t("settings.worldbook.regex") }}</option>
                </select>
                <button
                  type="button"
                  :class="button"
                  @click="current.activation.keys.splice(index, 1)"
                >
                  {{ t("settings.worldbook.removeKey") }}
                </button>
              </div>
              <input
                v-if="typeof key === 'string'"
                v-model="current.activation.keys[index]"
                :class="field"
                :aria-label="t('settings.worldbook.literal')"
              />
              <template v-else>
                <input
                  v-model="key.pattern"
                  :class="[field, 'font-mono']"
                  :aria-label="t('settings.worldbook.regex')"
                  @blur="check()"
                />
                <label class="flex items-center gap-2 text-xs"
                  ><input v-model="key.case_sensitive" type="checkbox" @change="check()" />{{
                    t("settings.worldbook.caseSensitive")
                  }}</label
                >
              </template>
              <p
                v-for="(issue, issueIndex) in entryIssues.filter(
                  (issue) => issue.key_index === index,
                )"
                :key="issueIndex"
                class="text-xs break-words whitespace-pre-wrap text-red-300"
              >
                {{ issue.message }}
              </p>
            </div>
            <button
              type="button"
              :class="button"
              :disabled="current.activation.keys.length >= 100"
              @click="current.activation.keys.push('')"
            >
              {{ t("settings.worldbook.addKey") }}
            </button>
            <p class="text-xs text-white/45">{{ t("settings.worldbook.regexHint") }}</p>
          </div>
          <label class="block space-y-1"
            >{{ t("settings.worldbook.content")
            }}<textarea v-model="current.content" rows="8" :class="[field, 'resize-y']" />
          </label>

          <p v-if="!current.content.trim()" class="text-xs text-amber-200">
            {{ t("settings.worldbook.blankContent") }}
          </p>
          <p
            v-for="(issue, index) in entryIssues.filter((issue) => issue.key_index == null)"
            :key="index"
            role="alert"
            class="text-xs text-red-300"
          >
            {{ issue.message }}
          </p>
        </div>
      </div>
      <p
        v-for="(issue, index) in issues.filter((issue) => issue.entry_index == null)"
        :key="index"
        role="alert"
        class="text-red-300"
      >
        {{ issue.message }}
      </p>
      <div class="space-y-3 rounded-xl border border-white/10 p-3">
        <label class="block space-y-1"
          >{{ t("settings.worldbook.testTitle")
          }}<textarea
            v-model="input"
            rows="3"
            :class="field"
            :placeholder="t('settings.worldbook.testHint')"
          />
        </label>
        <button type="button" :class="button" :disabled="testing" @click="check(true)">
          {{ t(testing ? "settings.worldbook.testing" : "settings.worldbook.test") }}
        </button>
        <ol v-if="results" class="space-y-2">
          <li
            v-for="result in results"
            :key="result.entry_index"
            class="rounded-lg bg-white/5 p-2 text-xs"
          >
            <button
              type="button"
              class="text-cyan-200 underline"
              @click="selected = result.entry_index"
            >
              {{ book.entries[result.entry_index]?.name || result.id }}
            </button>
            · {{ t(`settings.worldbook.status.${result.status}`) }}
            <span v-if="result.matched_keys.length">
              · {{ t("settings.worldbook.matchedKeys", { keys: matchedKeyLabels(result) }) }}</span
            >
          </li>
        </ol>
      </div>
    </template>
    <p v-if="error" role="alert" class="text-red-300">{{ error }}</p>
  </section>
</template>
