<template>
  <dialog
    ref="dialog"
    aria-labelledby="new-character-title"
    class="fixed m-auto w-full max-w-md bg-transparent p-4 backdrop:bg-black/60 backdrop:backdrop-blur-sm"
    @click.self="close"
    @cancel.prevent="close"
  >
    <form
      class="w-full max-w-md space-y-5 rounded-2xl border border-white/20 bg-slate-900 p-6 text-white shadow-2xl"
      @submit.prevent="submit"
    >
      <h2 id="new-character-title" class="text-xl font-bold">
        {{ t("settings.character.new.title") }}
      </h2>
      <p class="text-sm text-white/70">{{ t("settings.character.new.hint") }}</p>
      <label class="block space-y-2">
        <span>{{ t("settings.character.new.name") }}</span>
        <input
          ref="nameInput"
          v-model="name"
          required
          maxlength="100"
          :disabled="creating"
          class="w-full rounded-xl border border-white/20 bg-white/10 px-3 py-2"
        />
      </label>
      <label class="block space-y-2">
        <span>{{ t("settings.character.new.folder") }}</span>
        <input
          v-model="folder"
          required
          maxlength="80"
          :disabled="creating"
          class="w-full rounded-xl border border-white/20 bg-white/10 px-3 py-2"
        />
      </label>
      <p v-if="error" role="alert" class="text-sm text-rose-300">{{ error }}</p>
      <div class="flex justify-end gap-3">
        <button
          type="button"
          :disabled="creating"
          class="rounded-xl px-4 py-2 hover:bg-white/10 disabled:opacity-50"
          @click="close"
        >
          {{ t("settings.character.new.cancel") }}
        </button>
        <button
          type="submit"
          :disabled="creating || !name.trim() || !folder.trim()"
          class="rounded-xl bg-indigo-500 px-4 py-2 hover:bg-indigo-400 disabled:opacity-50"
        >
          {{ t(creating ? "settings.character.new.creating" : "settings.character.new.confirm") }}
        </button>
      </div>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { createCharacter, type CreatedCharacter } from "@/api/services/character";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ close: []; created: [character: CreatedCharacter] }>();
const { t } = useI18n();
const name = ref("");
const folder = ref("");
const creating = ref(false);
const error = ref("");
const nameInput = ref<HTMLInputElement>();
const dialog = ref<HTMLDialogElement>();
watch(
  () => props.visible,
  async (visible) => {
    if (!visible) {
      dialog.value?.close();
      return;
    }
    name.value = "";
    folder.value = `character-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    error.value = "";
    await nextTick();
    dialog.value?.showModal();
    nameInput.value?.focus();
  },
);
function close() {
  if (!creating.value) emit("close");
}
async function submit() {
  if (creating.value || !name.value.trim() || !folder.value.trim()) return;
  creating.value = true;
  error.value = "";
  try {
    const character = await createCharacter(name.value.trim(), folder.value.trim());
    emit("created", character);
    emit("close");
  } catch (cause) {
    error.value =
      typeof cause === "string"
        ? cause
        : cause instanceof Error
          ? cause.message
          : t("api.character.createFailed");
  } finally {
    creating.value = false;
  }
}
</script>
