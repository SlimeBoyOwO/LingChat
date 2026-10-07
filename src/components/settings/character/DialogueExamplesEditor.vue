<template>
  <section class="space-y-3 rounded-xl border border-white/10 bg-white/5 p-4">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="flex gap-1 rounded-lg bg-black/20 p-1">
        <button
          type="button"
          class="example-button"
          :class="{ 'bg-white/15! text-cyan-100!': !rawMode }"
          :aria-pressed="!rawMode"
          @click="rawMode = false"
        >
          {{ t("settings.dialogueExamples.cards") }}
        </button>
        <button
          type="button"
          class="example-button"
          :class="{ 'bg-white/15! text-cyan-100!': rawMode }"
          :aria-pressed="rawMode"
          @click="rawMode = true"
        >
          {{ t("settings.dialogueExamples.raw") }}
        </button>
      </div>
      <button
        v-if="!rawMode"
        type="button"
        class="example-button"
        :disabled="!history.length"
        @click="undo"
      >
        {{ t("settings.dialogueExamples.undo") }}
      </button>
    </div>
    <p class="text-xs leading-relaxed text-white/45">{{ t("settings.dialogueExamples.hint") }}</p>
    <textarea
      v-if="rawMode"
      :value="modelValue ?? ''"
      rows="10"
      class="example-control w-full resize-y font-mono leading-relaxed"
      :aria-label="t('settings.dialogueExamples.rawLabel')"
      @input="updateRaw(($event.target as HTMLTextAreaElement).value)"
    />
    <template v-else>
      <label v-if="draft.leading.trim()" class="block space-y-2 text-xs text-white/60">
        <span>{{ t("settings.dialogueExamples.leading") }}</span>
        <textarea
          v-model="draft.leading"
          rows="2"
          class="example-control w-full resize-y"
          @input="publish"
        />
      </label>
      <p
        v-if="!draft.messages.length"
        class="rounded-lg border border-dashed border-white/10 p-5 text-center text-sm text-white/40"
      >
        {{ t("settings.dialogueExamples.empty") }}
      </p>
      <article
        v-for="(message, index) in draft.messages"
        :key="message.id"
        class="space-y-3 rounded-xl border p-3"
        :class="
          message.role === 'user'
            ? 'border-cyan-300/15 bg-cyan-400/5'
            : 'border-violet-300/15 bg-violet-400/5'
        "
      >
        <div class="flex flex-wrap items-center gap-2">
          <span class="text-xs text-white/40">{{ index + 1 }}</span>
          <select
            :value="message.role"
            class="example-control min-w-0 flex-1"
            :aria-label="t('settings.dialogueExamples.roleLabel', { index: index + 1 })"
            @change="changeRole(message, ($event.target as HTMLSelectElement).value as ExampleRole)"
          >
            <option value="user">
              {{
                userName
                  ? `${userName} · ${t("settings.dialogueExamples.user")}`
                  : t("settings.dialogueExamples.user")
              }}
            </option>
            <option value="assistant">
              {{
                characterName
                  ? `${characterName} · ${t("settings.dialogueExamples.character")}`
                  : t("settings.dialogueExamples.character")
              }}
            </option>
          </select>
          <div class="flex gap-1">
            <button
              type="button"
              class="example-button"
              :disabled="index === 0"
              :aria-label="t('settings.dialogueExamples.moveUp', { index: index + 1 })"
              @click="move(index, -1)"
            >
              ↑
            </button>
            <button
              type="button"
              class="example-button"
              :disabled="index === draft.messages.length - 1"
              :aria-label="t('settings.dialogueExamples.moveDown', { index: index + 1 })"
              @click="move(index, 1)"
            >
              ↓
            </button>
            <button
              type="button"
              class="example-button text-red-200!"
              :aria-label="t('settings.dialogueExamples.remove', { index: index + 1 })"
              @click="remove(index)"
            >
              ×
            </button>
          </div>
        </div>
        <textarea
          :ref="(element) => registerInput(message.id, element as HTMLTextAreaElement | null)"
          v-model="message.content"
          rows="3"
          class="example-control w-full resize-y leading-relaxed"
          :aria-label="t('settings.dialogueExamples.contentLabel', { index: index + 1 })"
          :placeholder="t('settings.dialogueExamples.contentPlaceholder')"
          @input="publish"
        />
      </article>
      <div class="flex flex-wrap gap-2">
        <button type="button" class="example-button" @click="add('user')">
          {{ t("settings.dialogueExamples.addUser") }}
        </button>
        <button type="button" class="example-button" @click="add('assistant')">
          {{ t("settings.dialogueExamples.addCharacter") }}
        </button>
      </div>
    </template>
  </section>
</template>

<script setup lang="ts">
import { nextTick, ref, toRaw, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  parseDialogueExamples,
  serializeDialogueExamples,
  renumberExamples,
  setExampleRole,
  type ExampleDocument,
  type ExampleMessage,
  type ExampleRole,
} from "@/utils/dialogue-examples";

const props = defineProps<{
  modelValue?: string | null;
  userName?: string;
  characterName?: string;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const { t } = useI18n();
interface Card extends ExampleMessage {
  id: number;
}
type CardDocument = Omit<ExampleDocument, "messages"> & { messages: Card[] };
let nextId = 0;
let lastEmitted: string | null = null;
const inputs = new Map<number, HTMLTextAreaElement>();
const rawMode = ref(false);
const history = ref<CardDocument[]>([]);
function parse(text: string): CardDocument {
  const document = parseDialogueExamples(text, props.userName, props.characterName);
  return {
    ...document,
    messages: document.messages.map((message) => ({ ...message, id: ++nextId })),
  };
}
const draft = ref<CardDocument>(parse(props.modelValue ?? ""));
watch(
  () => props.modelValue,
  (value) => {
    if (value === lastEmitted) return;
    draft.value = parse(value ?? "");
    history.value = [];
  },
);
watch(
  () => [props.userName, props.characterName],
  () => {
    draft.value = parse(props.modelValue ?? "");
    history.value = [];
  },
);
function registerInput(id: number, element: HTMLTextAreaElement | null) {
  if (element) inputs.set(id, element);
  else inputs.delete(id);
}
function publish() {
  const value = serializeDialogueExamples(draft.value);
  lastEmitted = value;
  emit("update:modelValue", value);
}
function checkpoint() {
  history.value.push(structuredClone(toRaw(draft.value)));
  if (history.value.length > 20) history.value.shift();
}
function updateRaw(value: string) {
  lastEmitted = value;
  draft.value = parse(value);
  history.value = [];
  emit("update:modelValue", value);
}
async function add(role: ExampleRole) {
  checkpoint();
  const card: Card = {
    id: ++nextId,
    role,
    prefix: role === "user" ? "用户：" : "角色：",
    content: "",
    separator: "",
  };
  draft.value.messages.push(card);
  renumberExamples(draft.value.messages);
  publish();
  await nextTick();
  inputs.get(card.id)?.focus();
}
function move(index: number, direction: number) {
  const target = index + direction;
  if (target < 0 || target >= draft.value.messages.length) return;
  checkpoint();
  const [card] = draft.value.messages.splice(index, 1);
  draft.value.messages.splice(target, 0, card!);
  renumberExamples(draft.value.messages);
  publish();
}
function remove(index: number) {
  checkpoint();
  draft.value.messages.splice(index, 1);
  renumberExamples(draft.value.messages);
  publish();
}
function changeRole(message: Card, role: ExampleRole) {
  checkpoint();
  setExampleRole(message, role);
  publish();
}
function undo() {
  const previous = history.value.pop();
  if (!previous) return;
  draft.value = previous;
  publish();
}
</script>

<style scoped>
.example-button {
  border-radius: 0.5rem;
  background: rgb(255 255 255 / 0.06);
  padding: 0.4rem 0.65rem;
  font-size: 0.75rem;
  color: rgb(255 255 255 / 0.7);
}
.example-button:hover:not(:disabled) {
  background: rgb(255 255 255 / 0.15);
}
.example-button:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}
.example-control {
  border: 1px solid rgb(255 255 255 / 0.1);
  border-radius: 0.5rem;
  background: rgb(0 0 0 / 0.2);
  padding: 0.5rem 0.65rem;
  color: white;
  font-size: 0.8rem;
}
.example-control option {
  background: #292929;
}
</style>
