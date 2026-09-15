<template>
  <!-- 好感度/负面情绪变化飘出标签：右侧居中纵向堆叠，滑入→停留→淡出，不拦截点击 -->
  <div class="absolute top-1/2 right-6 flex -translate-y-1/2 flex-col items-end gap-1.5">
    <div
      v-for="(chip, i) in chips"
      :key="`${batchId}-${i}`"
      class="affection-float-chip"
      :class="chip.colorClass"
      :style="{ animationDelay: `${i * STAGGER_MS}ms` }"
    >
      <span v-if="chip.roleName" class="opacity-60">{{ chip.roleName }}&nbsp;</span>
      {{ chip.label }} {{ chip.delta > 0 ? "+" : "" }}{{ chip.delta }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useGameStore } from "@/stores/modules/game";

const { t } = useI18n();
const gameStore = useGameStore();

interface FloatChip {
  label: string;
  delta: number;
  colorClass: string;
  /** 多角色场景：变化不属于当前对话角色时为该角色名（否则空串） */
  roleName: string;
}

const chips = ref<FloatChip[]>([]);
let batchId = 0;
let clearTimer: number | null = null;

const POS_DIMS = ["fondness", "trust", "intimacy", "rapport", "interest", "longing"] as const;
const NEG_DIMS = [
  "anger",
  "hurt",
  "disappointment",
  "indifference",
  "jealousy",
  "estrangement",
] as const;

const STAGGER_MS = 80;
/** 单个 chip 动画时长（滑入 + 停留 + 淡出），与 CSS keyframes 一致 */
const LIFE_MS = 2200;

watch(
  () => gameStore.lastAffectionChange,
  (change) => {
    if (!change) return;
    const roleName =
      change.roleId !== gameStore.currentInteractRoleId
        ? (gameStore.gameRoles[change.roleId]?.roleName ?? "")
        : "";
    const next: FloatChip[] = [];
    for (const key of POS_DIMS) {
      const d = change.deltas[key];
      if (!d) continue;
      next.push({
        label: t(`ui.affection.${key}`),
        delta: d,
        colorClass: d > 0 ? "af-pos-up" : "af-pos-down",
        roleName,
      });
    }
    for (const key of NEG_DIMS) {
      const d = change.negativeDeltas[key];
      if (!d) continue;
      next.push({
        label: t(`ui.affection.neg.${key}`),
        delta: d,
        colorClass: d > 0 ? "af-neg-up" : "af-neg-down",
        roleName,
      });
    }
    if (next.length === 0) return;
    // 新一轮变化到达时直接替换旧批次
    if (clearTimer !== null) window.clearTimeout(clearTimer);
    batchId += 1;
    chips.value = next;
    clearTimer = window.setTimeout(
      () => {
        chips.value = [];
        clearTimer = null;
      },
      LIFE_MS + STAGGER_MS * next.length,
    );
  },
);

onUnmounted(() => {
  if (clearTimer !== null) window.clearTimeout(clearTimer);
});
</script>

<style scoped>
/* 飘出 chip：玻璃拟态小 pill；一次 CSS 动画完成 滑入→停留→淡出，
     animation-delay 由模板按序 stagger（backwards fill 保证延迟期间不可见） */
.affection-float-chip {
  padding: 3px 12px;
  border-radius: 9999px;
  font-size: 13px;
  font-weight: 700;
  line-height: 1.5;
  white-space: nowrap;
  color: #fff;
  background: rgba(18, 18, 28, 0.78);
  backdrop-filter: blur(12px);
  border: 1px solid rgba(255, 255, 255, 0.14);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
  opacity: 0;
  animation: affection-float-chip 2.2s cubic-bezier(0.2, 0.8, 0.2, 1) both;
}
/* 好感：正粉负蓝；负面：向增暗红、向减青绿（消解）——与 AffectionPanel 最近变化区一致 */
.af-pos-up {
  color: #ff9ec7;
  border-color: rgba(255, 158, 199, 0.45);
  text-shadow: 0 0 8px rgba(255, 105, 180, 0.5);
}
.af-pos-down {
  color: #7fc4ff;
  border-color: rgba(127, 196, 255, 0.4);
}
.af-neg-up {
  color: #f06292;
  border-color: rgba(240, 98, 146, 0.45);
  text-shadow: 0 0 8px rgba(224, 65, 110, 0.5);
}
.af-neg-down {
  color: #8fd6a8;
  border-color: rgba(143, 214, 168, 0.4);
}

@keyframes affection-float-chip {
  0% {
    opacity: 0;
    transform: translateX(28px);
  }
  12% {
    opacity: 1;
    transform: translateX(0);
  }
  78% {
    opacity: 1;
    transform: translateX(0);
  }
  100% {
    opacity: 0;
    transform: translateX(10px);
  }
}
</style>
