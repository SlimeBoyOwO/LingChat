<template>
  <div class="space-y-4">
    <p class="text-sm leading-relaxed text-white/50">{{ t("settings.costumes.hint") }}</p>
    <div class="flex flex-wrap gap-2">
      <input
        v-model="newName"
        :disabled="busy"
        class="min-w-0 flex-1 rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white"
        :placeholder="t('settings.costumes.newName')"
        :aria-label="t('settings.costumes.newName')"
        @keydown.enter="apply('create', '', newName)"
      />
      <button
        type="button"
        :disabled="busy || !newName.trim()"
        class="rounded-lg bg-[#5e72e4] px-4 py-2 text-sm text-white hover:bg-[#4a5acf] disabled:opacity-40"
        @click="apply('create', '', newName)"
      >
        {{ t("settings.costumes.add") }}
      </button>
    </div>
    <p v-if="error" role="alert" class="text-sm text-red-300">{{ error }}</p>
    <p v-if="loading" class="text-sm text-white/50">{{ t("settings.shared.loading") }}</p>
    <article
      v-for="costume in costumes"
      :key="costume.name"
      class="flex flex-col gap-4 rounded-xl border border-white/10 bg-white/5 p-4 sm:flex-row"
    >
      <div
        class="flex aspect-[3/4] w-28 shrink-0 items-center justify-center self-start overflow-hidden rounded-lg bg-black/20"
      >
        <img
          v-if="costume.preview"
          :src="`${convertFileSrc(costume.preview)}?v=${revision}`"
          :alt="costume.name === 'default' ? t('settings.avatars.default') : costume.name"
          class="h-full w-full object-contain"
        />
        <span v-else class="px-2 text-center text-xs text-white/35">{{
          t("settings.costumes.noPreview")
        }}</span>
      </div>
      <div class="min-w-0 flex-1 space-y-3">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <strong class="text-sm text-white/85">{{
            costume.name === "default" ? t("settings.avatars.default") : costume.name
          }}</strong>
          <span
            class="text-xs"
            :class="costume.present === costume.total ? 'text-cyan-200' : 'text-amber-200/80'"
            >{{
              t("settings.costumes.complete", { present: costume.present, total: costume.total })
            }}</span
          >
        </div>
        <div v-if="costume.name !== 'default'" class="flex gap-2">
          <input
            v-model="names[costume.name]"
            :disabled="busy"
            class="min-w-0 flex-1 rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white"
            :aria-label="t('settings.costumes.renameName', { name: costume.name })"
          />
          <button
            type="button"
            :disabled="
              busy || !names[costume.name]?.trim() || names[costume.name].trim() === costume.name
            "
            class="rounded-lg bg-white/10 px-3 text-xs text-white/70 hover:bg-white/15 disabled:opacity-30"
            @click="apply('rename', costume.name, names[costume.name])"
          >
            {{ t("settings.costumes.rename") }}
          </button>
          <button
            type="button"
            :disabled="busy"
            class="rounded-lg bg-red-500/15 px-3 text-xs text-red-200 hover:bg-red-500/25 disabled:opacity-30"
            @click="remove(costume.name)"
          >
            {{ t("settings.costumes.remove") }}
          </button>
        </div>
        <label v-if="costume.name !== 'default'" class="flex flex-col gap-2 text-xs text-white/60">
          {{ t("settings.costumes.prompt") }}
          <textarea
            :value="promptFor(costume.name)"
            :disabled="busy"
            rows="2"
            class="rounded-lg border border-white/10 bg-black/20 px-3 py-2 text-sm text-white"
            @input="setPrompt(costume.name, ($event.target as HTMLTextAreaElement).value)"
          ></textarea>
        </label>
        <label v-if="modelValue.live2d" class="flex flex-col gap-2 text-xs text-white/60">
          {{ t("settings.costumes.model") }}
          <select
            :value="modelValue.live2d.clothes_variants?.[costume.name] ?? ''"
            :disabled="busy"
            class="rounded-lg border border-white/10 bg-[#252535] px-3 py-2 text-sm text-white"
            @change="setVariant(costume.name, ($event.target as HTMLSelectElement).value)"
          >
            <option value="">{{ t("settings.costumes.defaultModel") }}</option>
            <option
              v-for="variant in Object.keys(modelValue.live2d.variants ?? {})"
              :key="variant"
              :value="variant"
            >
              {{ variant }}
            </option>
          </select>
        </label>
        <details v-if="costume.missing.length" class="text-xs text-white/45">
          <summary class="cursor-pointer hover:text-white/70">
            {{ t("settings.costumes.missing") }}
          </summary>
          <p class="mt-2 leading-relaxed">{{ costume.missing.join("、") }}</p>
        </details>
        <button
          type="button"
          class="text-xs text-cyan-200/80 hover:text-cyan-100"
          @click="emit('edit-avatars', costume.name)"
        >
          {{ t("settings.costumes.editAvatars") }}
        </button>
      </div>
    </article>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, toRaw } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import {
  listCharacterCostumes,
  manageCharacterCostume,
  type CharacterCostumeSummary,
} from "@/api/services/character";
import { useDialogStore } from "@/stores/modules/ui/dialog";

const props = defineProps<{ roleId: number; modelValue: Record<string, any> }>();
const emit = defineEmits<{
  "update:modelValue": [value: Record<string, any>];
  changed: [value: { oldName: string; newName: string }];
  "edit-avatars": [name: string];
}>();
const { t } = useI18n();
const dialog = useDialogStore();
const costumes = ref<CharacterCostumeSummary[]>([]);
const names = ref<Record<string, string>>({});
const newName = ref("");
const busy = ref(false);
const loading = ref(false);
const error = ref("");
const revision = ref(Date.now());
let generation = 0;
async function load() {
  const current = ++generation;
  loading.value = true;
  try {
    const result = await listCharacterCostumes(props.roleId);
    if (current !== generation) return;
    costumes.value = result;
    names.value = Object.fromEntries(result.map((c) => [c.name, c.name]));
    revision.value = Date.now();
  } catch (e) {
    if (current === generation) error.value = String(e);
  } finally {
    if (current === generation) loading.value = false;
  }
}
watch(() => props.roleId, load, { immediate: true });
function promptFor(name: string) {
  return props.modelValue.clothes?.find((c: { name: string }) => c.name === name)?.prompt ?? "";
}
function setPrompt(name: string, prompt: string) {
  const clothes = [...(props.modelValue.clothes ?? [])];
  const index = clothes.findIndex((c) => c.name === name);
  if (index < 0) clothes.push({ name, prompt });
  else clothes[index] = { ...clothes[index], prompt };
  emit("update:modelValue", { ...props.modelValue, clothes });
}
function setVariant(name: string, variant: string) {
  const live2d = structuredClone(toRaw(props.modelValue.live2d));
  live2d.clothes_variants ??= {};
  if (variant) live2d.clothes_variants[name] = variant;
  else delete live2d.clothes_variants[name];
  emit("update:modelValue", { ...props.modelValue, live2d });
}
async function apply(action: "create" | "rename" | "remove", old: string, name: string) {
  if (busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    const settings = await manageCharacterCostume(
      props.roleId,
      action,
      old,
      name.trim(),
      props.modelValue,
    );
    emit("update:modelValue", settings);
    newName.value = "";
    await load();
    emit("changed", { oldName: old, newName: action === "remove" ? "default" : name.trim() });
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
async function remove(name: string) {
  if (
    await dialog.confirm(
      t("settings.costumes.confirmRemove", { name }),
      t("settings.costumes.remove"),
    )
  )
    await apply("remove", name, "");
}
</script>
