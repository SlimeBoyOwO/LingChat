<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 scale-95"
      enter-to-class="opacity-100 scale-100"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100 scale-100"
      leave-to-class="opacity-0 scale-95"
    >
      <div
        v-if="visible"
        class="fixed inset-0 z-10000 flex items-center justify-center p-4"
        @click="emit('close')"
      >
        <div class="absolute inset-0 bg-slate-900/60 backdrop-blur-sm"></div>

        <div
          class="relative z-10 w-full max-w-md rounded-2xl border border-white/15 bg-slate-900/90 p-6 text-white shadow-2xl backdrop-blur-xl"
          @click.stop
        >
          <h3 class="text-lg leading-snug font-bold">
            {{ $t(titleKey, { name: firstName, me: meName }) }}
          </h3>
          <p class="mt-3 text-sm leading-relaxed text-white/70">
            {{ $t("game.relationGuard.modalBodyMulti", { me: meName }) }}
          </p>
          <!-- 逐行列出缺关系的角色与缺哪一侧 -->
          <ul class="mt-3 space-y-1.5">
            <li
              v-for="(item, idx) in roles"
              :key="`${item.name}-${idx}`"
              class="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs"
            >
              <span class="truncate text-white/85">{{ item.name }}</span>
              <span class="shrink-0 text-white/50">{{ sideText(item.missingSide) }}</span>
            </li>
          </ul>
          <p class="mt-2 text-xs leading-relaxed text-white/45">
            {{ $t("game.relationGuard.example") }}
          </p>

          <div class="mt-6 flex justify-end gap-3">
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
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
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
