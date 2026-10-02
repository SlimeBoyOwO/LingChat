<template>
  <div class="mb-5 rounded-xl border border-white/10 bg-black/15 p-3">
    <div class="flex flex-wrap items-start gap-3">
      <div
        class="flex min-w-0 flex-1 flex-wrap gap-2"
        :aria-label="$t('settings.background.scene.title')"
      >
        <button
          v-for="category in [ALL_CATEGORY, ...categories]"
          :key="category"
          type="button"
          class="flex max-w-full items-center gap-1.5 rounded-lg border px-3 py-2 text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-300"
          :class="
            modelValue === category
              ? 'border-brand/50 bg-brand/15 text-brand'
              : 'border-transparent bg-white/5 text-white/65 hover:bg-white/10 hover:text-white'
          "
          :aria-pressed="modelValue === category"
          :title="category"
          @click="$emit('update:modelValue', category)"
        >
          <component
            :is="category === VIRTUAL_CATEGORY ? Puzzle : Folder"
            :size="14"
            class="shrink-0"
          />
          <span class="truncate">{{
            category === ALL_CATEGORY
              ? $t("settings.background.scene.categoryAll")
              : category === ROOT_CATEGORY
                ? $t("settings.background.scene.moveToRoot")
                : category
          }}</span>
        </button>
      </div>
      <div class="flex shrink-0 flex-wrap items-center gap-2">
        <button
          type="button"
          class="flex items-center gap-1.5 rounded-lg border border-white/15 px-3 py-2 text-sm text-white/80 transition-colors hover:bg-white/10 disabled:opacity-40"
          :aria-expanded="showCreate"
          :disabled="busy"
          @click="openCreate"
        >
          <Plus :size="15" />
          {{ $t("settings.background.scene.categoryAdd") }}
        </button>
        <button
          v-if="
            modelValue !== ALL_CATEGORY &&
            modelValue !== VIRTUAL_CATEGORY &&
            modelValue !== ROOT_CATEGORY
          "
          type="button"
          class="flex items-center gap-1.5 rounded-lg border border-red-400/20 px-3 py-2 text-sm text-red-300 transition-colors hover:bg-red-500/10 disabled:opacity-40"
          :disabled="busy"
          @click="$emit('delete')"
        >
          <Trash2 :size="14" />
          {{ $t("settings.background.scene.categoryDelete") }}
        </button>
      </div>
    </div>
    <form
      v-if="showCreate"
      class="mt-3 flex flex-wrap items-center gap-2 border-t border-white/10 pt-3"
      @submit.prevent="$emit('create')"
    >
      <input
        ref="nameInput"
        :value="name"
        :placeholder="$t('settings.background.scene.categoryNamePlaceholder')"
        :aria-label="$t('settings.background.scene.categoryNamePlaceholder')"
        :disabled="busy"
        class="focus:border-brand min-w-0 flex-1 rounded-lg border border-white/15 bg-black/20 px-3 py-2 text-sm text-white placeholder:text-white/35 focus:outline-none sm:max-w-64"
        @input="$emit('update:name', ($event.target as HTMLInputElement).value)"
        @keydown.esc.prevent="closeCreate"
      />
      <button
        type="submit"
        :disabled="busy || !name.trim()"
        class="bg-brand/80 hover:bg-brand rounded-lg px-3 py-2 text-sm font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-40"
      >
        <LoaderCircle v-if="busy" :size="16" class="animate-spin" />
        <span v-else>{{ $t("settings.background.scene.categoryAdd") }}</span>
      </button>
      <button
        type="button"
        :disabled="busy"
        class="rounded-lg p-2 text-white/50 hover:bg-white/10 hover:text-white"
        :aria-label="$t('common.cancel')"
        @click="closeCreate"
      >
        <X :size="18" />
      </button>
    </form>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { Folder, Puzzle, Plus, Trash2, X, LoaderCircle } from "lucide-vue-next";
import { ALL_CATEGORY, ROOT_CATEGORY, VIRTUAL_CATEGORY } from "@/constants/background-categories";

const props = defineProps<{
  modelValue: string;
  categories: string[];
  name: string;
  busy: boolean;
}>();
const emit = defineEmits<{
  (e: "update:modelValue", value: string): void;
  (e: "update:name", value: string): void;
  (e: "create"): void;
  (e: "delete"): void;
}>();
const showCreate = ref(false);
const nameInput = ref<HTMLInputElement | null>(null);
async function openCreate() {
  showCreate.value = !showCreate.value;
  if (showCreate.value) {
    await nextTick();
    nameInput.value?.focus();
  }
}
function closeCreate() {
  showCreate.value = false;
  emit("update:name", "");
}
// 父组件仅在创建成功后清空名字；失败时保留表单供用户修正。
watch(
  () => props.name,
  (name, previous) => {
    if (previous && !name) showCreate.value = false;
  },
);
</script>
