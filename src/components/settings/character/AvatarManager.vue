<template>
  <div class="space-y-4">
    <p class="text-sm leading-relaxed text-white/50">{{ t("settings.avatars.hint") }}</p>
    <label class="flex items-center gap-3 text-sm text-white/70">
      {{ t("settings.avatars.costume") }}
      <select
        v-model="costume"
        :disabled="busy"
        class="rounded-xl border border-white/10 bg-[#252535] px-3 py-2 text-white"
      >
        <option value="default">{{ t("settings.avatars.default") }}</option>
        <option v-for="name in costumeNames" :key="name" :value="name">{{ name }}</option>
      </select>
    </label>
    <p v-if="error" role="alert" class="text-sm text-red-300">{{ error }}</p>
    <p v-if="loading" class="text-sm text-white/50">{{ t("settings.shared.loading") }}</p>
    <div v-else class="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
      <article
        v-for="slot in slots"
        :key="slot.emotion"
        class="space-y-2 rounded-xl border border-white/10 bg-white/5 p-3"
      >
        <div class="flex flex-wrap items-center justify-between gap-1 text-xs">
          <span class="text-white/80">{{ emotionLabel(slot.emotion) }}</span>
          <span :class="slot.path ? 'text-cyan-200/70' : 'text-amber-200/70'">{{
            t(
              slot.fallback
                ? "settings.avatars.fallback"
                : slot.path
                  ? "settings.avatars.present"
                  : "settings.avatars.missing",
            )
          }}</span>
        </div>
        <div
          class="flex aspect-[3/4] items-center justify-center overflow-hidden rounded-lg bg-black/20"
        >
          <img
            v-if="slot.path"
            :src="imageUrl(slot.path)"
            :alt="emotionLabel(slot.emotion)"
            class="h-full w-full object-contain"
          />
          <span v-else class="text-xs text-white/30">{{ t("settings.avatars.missing") }}</span>
        </div>
        <div class="flex gap-2 text-xs">
          <label
            class="flex-1 rounded-lg bg-[#5e72e4] px-2 py-2 text-center"
            :class="busy ? 'pointer-events-none opacity-50' : 'cursor-pointer hover:bg-[#4a5acf]'"
          >
            {{
              t(
                slot.path && !slot.fallback
                  ? "settings.avatars.replace"
                  : "settings.avatars.upload",
              )
            }}
            <input
              type="file"
              accept="image/png,image/jpeg,image/webp"
              class="sr-only"
              :disabled="busy"
              @change="upload(slot, $event)"
            />
          </label>
          <button
            :disabled="busy || !slot.path || slot.fallback"
            class="rounded-lg bg-red-500/15 px-2 py-2 text-red-200 hover:bg-red-500/30 disabled:opacity-30"
            @click="remove(slot)"
          >
            {{ t("settings.avatars.delete") }}
          </button>
        </div>
      </article>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useCharacterEditorApi } from "@/composables/useCharacterEditor";
const { listCharacterAvatars, writeCharacterAvatar, deleteCharacterAvatar } =
  useCharacterEditorApi();
import { computed, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { type CharacterAvatarSlot } from "@/api/services/character";
import { useI18n } from "vue-i18n";
import { useDialogStore } from "@/stores/modules/ui/dialog";

const props = defineProps<{
  roleId: number;
  clothes: Array<{ name?: string }>;
  resourceClothes?: Array<{ title: string }>;
  initialCostume?: string;
}>();
const emit = defineEmits<{ changed: [] }>();
const { t } = useI18n();
const dialog = useDialogStore();
type Slot = CharacterAvatarSlot;
const costume = ref(props.initialCostume || "default");
const costumeNames = computed(() => [
  ...new Set(
    [...props.clothes.map((c) => c.name), ...(props.resourceClothes ?? []).map((c) => c.title)]
      .map((name) => name?.trim())
      .filter((n): n is string => Boolean(n) && n !== "default" && n !== "默认"),
  ),
]);
const slots = ref<Slot[]>([]);
const loading = ref(false);
const busy = ref(false);
const error = ref("");
const revision = ref(Date.now());
let generation = 0;
const imageUrl = (path: string) => `${convertFileSrc(path)}?v=${revision.value}`;
const emotionLabel = (emotion: string) =>
  emotion === "伤心"
    ? t("settings.avatars.sadAlias")
    : emotion === "羞耻"
      ? t("settings.avatars.shyAlias")
      : emotion;

async function load() {
  const current = ++generation;
  loading.value = true;
  error.value = "";
  try {
    const result = await listCharacterAvatars(props.roleId, costume.value);
    if (current === generation) {
      slots.value = result;
      revision.value = Date.now();
    }
  } catch (e) {
    if (current === generation) error.value = String(e);
  } finally {
    if (current === generation) loading.value = false;
  }
}
watch(() => [props.roleId, costume.value], load, { immediate: true });

async function upload(slot: Slot, event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  if (file.size > 20 * 1024 * 1024) {
    error.value = t("settings.avatars.tooLarge");
    return;
  }
  busy.value = true;
  error.value = "";
  try {
    await writeCharacterAvatar(
      props.roleId,
      costume.value,
      slot.emotion,
      Array.from(new Uint8Array(await file.arrayBuffer())),
    );
    await load();
    emit("changed");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}

async function remove(slot: Slot) {
  const roleId = props.roleId;
  const clothes = costume.value;
  if (
    !(await dialog.confirm(
      t("settings.avatars.confirmDelete", { emotion: emotionLabel(slot.emotion) }),
      t("settings.avatars.delete"),
    ))
  )
    return;
  busy.value = true;
  error.value = "";
  try {
    await deleteCharacterAvatar(roleId, clothes, slot.emotion);
    await load();
    emit("changed");
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = false;
  }
}
</script>
