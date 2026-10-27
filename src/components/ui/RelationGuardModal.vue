<template>
  <BaseModal :show="visible" :title="title" theme="glass" @close="emit('close')">
    <p class="text-sm leading-relaxed text-white/70">
      {{ $t("game.relationGuard.modalBodyMulti", { me: meName }) }}
    </p>
    <!-- 逐行列出缺关系的角色与缺哪一侧 -->
    <ul class="space-y-1.5">
      <li
        v-for="(item, idx) in roles"
        :key="`${item.name}-${idx}`"
        class="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs"
      >
        <span class="truncate text-white/85">{{ item.name }}</span>
        <span class="shrink-0 text-white/50">{{ sideText(item.missingSide) }}</span>
      </li>
    </ul>
    <p class="text-xs leading-relaxed text-white/45">
      {{ $t("game.relationGuard.example") }}
    </p>

    <template #footer>
      <div class="flex justify-end gap-3">
        <button
          class="cursor-pointer rounded-full border border-white/10 bg-white/10 px-4 py-1.5 text-xs font-semibold text-white transition-all hover:bg-white/20"
          @click="emit('close')"
        >
          {{ $t("game.relationGuard.later") }}
        </button>
        <button
          class="text-brand cursor-pointer rounded-full bg-indigo-600/80 px-4 py-1.5 text-xs font-semibold text-white shadow-lg shadow-indigo-500/20 transition-all hover:bg-indigo-500"
          @click="emit('gotoEdit')"
        >
          {{ $t("game.relationGuard.gotoEdit") }}
        </button>
      </div>
    </template>
  </BaseModal>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import BaseModal from "./BaseModal.vue";
import type { RelationMissingSide } from "@/composables/chat/useRelationGuard";

const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    visible: boolean;
    /** 缺关系的角色（顺序同后端），逐行展示；第一行决定标题文案 */
    roles?: { name: string; missingSide: RelationMissingSide }[];
    /** 玩家名，正文里说明「你对这个角色的说法」用 */
    meName?: string;
  }>(),
  { roles: () => [], meName: "我" },
);

const firstName = computed(() => props.roles[0]?.name ?? "");

const titleKey = computed(() => {
  switch (props.roles[0]?.missingSide) {
    case "role":
      return "game.relationGuard.modalTitleRole";
    case "identity":
      return "game.relationGuard.modalTitleIdentity";
    default:
      return "game.relationGuard.modalTitle";
  }
});

/** BaseModal 的 title 是字符串，在这里就把三处文案的插值算完，别指望模板里再传参数。 */
const title = computed(() => t(titleKey.value, { name: firstName.value, me: props.meName }));

const sideText = (side: RelationMissingSide) => {
  switch (side) {
    case "role":
      return t("game.relationGuard.sideRole");
    case "identity":
      return t("game.relationGuard.sideIdentity");
    default:
      return t("game.relationGuard.sideBoth");
  }
};

const emit = defineEmits(["close", "gotoEdit"]);
</script>
