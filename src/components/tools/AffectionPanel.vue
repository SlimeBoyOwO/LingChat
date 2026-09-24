<template>
  <div class="flex flex-col gap-3">
    <Button
      type="nav"
      :class="[
        'flex items-center gap-2 px-4 py-2 transition-colors',
        enabled ? 'text-[#ff8fc0]' : 'text-white',
      ]"
      @click="toggleEnabled"
      v-show="!uiStore.showSettings"
    >
      <HeartLiquid
        :value="average"
        :negative="negativePeak"
        :size="18"
        :wave="waveEnabled"
        :class="{ 'affection-heartbeat': heartbeatEnabled }"
        :style="heartbeatStyle"
      />
      <h3 class="m-0 hidden text-lg font-bold xl:block">
        {{ $t("ui.affection.title") }}
        <span v-if="average !== null" class="ml-1 text-sm font-normal tabular-nums opacity-80">
          {{ average }}
        </span>
      </h3>
    </Button>

    <!-- 日程式弹窗：全屏遮罩 + 居中窗口（双雷达，宽屏 880px / 窄屏近全宽） -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
        leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="enabled"
          class="fixed inset-0 z-[1100] flex items-center justify-center bg-black/50 backdrop-blur-sm"
          @click.self="close"
        >
          <Transition
            enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
            leave-active-class="transition-all duration-200 cubic-bezier(0.6, -0.28, 0.74, 0.05)"
            enter-from-class="opacity-0 scale-95 translate-y-2"
            leave-to-class="opacity-0 scale-95 translate-y-2"
          >
            <div
              v-if="enabled"
              class="relative flex max-h-[85dvh] flex-col overflow-hidden rounded-2xl border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)]"
              :class="uiStore.isNarrowScreen ? 'w-[95vw]' : 'w-168'"
            >
              <!-- Header bar -->
              <div
                class="flex shrink-0 items-center justify-between border-b border-white/10 bg-[#12121c]/90 px-4 py-2.5 backdrop-blur-xl"
              >
                <div class="flex items-center gap-2">
                  <!-- 面板内爱心常跳；心跳开关只控制顶栏按钮 -->
                  <HeartLiquid
                    :value="average"
                    :negative="negativePeak"
                    :size="17"
                    :wave="waveEnabled"
                    class="affection-heartbeat shrink-0"
                    :style="heartbeatStyle"
                  />
                  <h3 class="text-sm font-semibold tracking-wide text-white">
                    {{ $t("ui.affection.title") }}
                  </h3>
                </div>
                <button
                  class="rounded-full p-1.5 text-white/50 transition-colors hover:bg-white/10 hover:text-white"
                  @click="close"
                >
                  <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </button>
              </div>

              <!-- Content area with glass styling -->
              <div
                class="min-h-0 flex-1 scrollbar-thin [scrollbar-color:var(--accent-color)_transparent] overflow-y-auto rounded-b-2xl bg-[#12121c]/75 p-4 text-white backdrop-blur-[20px]"
              >
                <!-- 只显示当前对话角色；切换角色时旧内容淡出、新内容淡入（雷达子组件随 key 重挂载） -->
                <Transition name="affection-role" mode="out-in">
                  <div :key="role?.roleId ?? 'none'">
                    <template v-if="affection">
                      <!-- 角色头部：头像 + 名字 + 平均值大数字 + 档位徽章 -->
                      <div class="flex items-center gap-2.5">
                        <img
                          v-if="avatarUrl"
                          :src="avatarUrl"
                          :alt="role?.roleName ?? ''"
                          class="h-10 w-10 shrink-0 rounded-full border border-white/15 object-cover"
                        />
                        <div
                          v-else
                          class="h-10 w-10 shrink-0 rounded-full border border-white/10 bg-white/5"
                        ></div>
                        <div class="min-w-0 flex-1">
                          <div class="truncate text-base font-semibold">
                            {{ role?.roleName ?? "—" }}
                          </div>
                          <div class="text-[11px] text-white/50">
                            {{ $t("ui.affection.average") }}
                          </div>
                        </div>
                        <div class="flex shrink-0 flex-col items-end gap-1">
                          <span
                            class="text-2xl leading-none font-bold tabular-nums"
                            :style="{ color: tierColor }"
                          >
                            {{ average }}
                          </span>
                          <span
                            class="affection-tier-badge"
                            :class="{
                              'affection-tier-pulse': tier === 'overflow',
                              'affection-tier-alert': negativePeak > 60,
                            }"
                            :style="tierBadgeStyle"
                          >
                            {{ $t(`ui.affection.tier.${tier}`) }}
                          </span>
                        </div>
                      </div>

                      <!-- 当前负面情绪：由负面六维派生（强度 > 30 的维度），暗红系芯片 -->
                      <div
                        v-if="negativeChips.length > 0"
                        class="mt-2.5 flex flex-wrap items-center gap-1.5"
                      >
                        <span class="flex items-center gap-1 text-[11px] text-white/50">
                          <CloudRain :size="11" />
                          {{ $t("ui.affection.negativeTitle") }}
                        </span>
                        <span
                          v-for="chip in negativeChips"
                          :key="chip.key"
                          class="affection-neg-chip"
                        >
                          {{ $t(`ui.affection.neg.${chip.key}`) }} {{ chip.value }}
                        </span>
                      </div>

                      <!-- 距下一档进度 -->
                      <div
                        v-if="nextTierInfo"
                        class="mt-1.5 flex items-center justify-end gap-2 text-[11px] text-white/50"
                      >
                        <div class="h-1 w-20 overflow-hidden rounded-full bg-white/10">
                          <div
                            class="h-full rounded-full transition-[width] duration-700"
                            :style="{ width: `${nextTierInfo.progress}%`, background: tierColor }"
                          ></div>
                        </div>
                        <span>
                          {{
                            $t("ui.affection.nextTier", {
                              tier: $t(`ui.affection.tier.${nextTierInfo.key}`),
                              points: nextTierInfo.points,
                            })
                          }}
                        </span>
                      </div>
                      <div
                        v-else-if="tier === 'overflow'"
                        class="mt-2 text-right text-xs"
                        :style="{ color: tierColor }"
                      >
                        {{ $t("ui.affection.maxTier") }}
                      </div>

                      <!-- 双雷达：左好感 / 右负面；窄屏上下堆叠（窗口内容区可滚动） -->
                      <div class="mt-1 flex flex-col gap-1 md:flex-row md:gap-3">
                        <AffectionRadar
                          :title="$t('ui.affection.radarAffection')"
                          :values="affectionValues"
                          :labels="affectionLabels"
                          :descs="affectionDescs"
                          :palette="affectionPalette"
                        />
                        <AffectionRadar
                          v-if="negative"
                          :title="$t('ui.affection.radarNegative')"
                          :values="negativeValues"
                          :labels="negativeLabels"
                          :descs="negativeDescs"
                          :palette="negativePalette"
                        />
                      </div>

                      <!-- 最近一次评估变化（好感增量 + 负面增量并列；负面向增暗红、向减青绿=消解） -->
                      <div
                        v-if="recentChange"
                        class="mt-2.5 rounded-xl border border-white/10 bg-white/5 p-2.5"
                      >
                        <div class="mb-1 text-[11px] font-semibold text-white/60">
                          {{ $t("ui.affection.recentChange") }}
                        </div>
                        <div class="flex flex-wrap gap-1.5">
                          <span
                            v-for="entry in recentChange.entries"
                            :key="`pos-${entry.key}`"
                            class="affection-delta-chip"
                            :class="entry.delta > 0 ? 'affection-delta-up' : 'affection-delta-down'"
                          >
                            {{ $t(`ui.affection.${entry.key}`) }}
                            {{ entry.delta > 0 ? "+" : "" }}{{ entry.delta }}
                          </span>
                          <span
                            v-for="entry in recentChange.negEntries"
                            :key="`neg-${entry.key}`"
                            class="affection-delta-chip"
                            :class="entry.delta > 0 ? 'affection-neg-up' : 'affection-neg-down'"
                          >
                            {{ $t(`ui.affection.neg.${entry.key}`) }}
                            {{ entry.delta > 0 ? "+" : "" }}{{ entry.delta }}
                          </span>
                        </div>
                        <div
                          v-if="recentChange.reason"
                          class="mt-1 text-[11px] leading-relaxed text-white/50"
                        >
                          {{ recentChange.reason }}
                        </div>
                      </div>
                    </template>

                    <div v-else class="py-8 text-center text-sm text-white/40">
                      {{ $t("ui.affection.noData") }}
                    </div>

                    <!-- 好感度介绍（可折叠） -->
                    <div class="mt-3 border-t border-white/10 pt-2.5">
                      <button
                        class="flex w-full cursor-pointer items-center justify-between border-none bg-transparent p-0 text-xs text-white/60 transition-colors hover:text-white"
                        @click="introOpen = !introOpen"
                      >
                        <span>{{ $t("ui.affection.introTitle") }}</span>
                        <ChevronDown
                          :size="13"
                          class="transition-transform duration-200"
                          :class="{ 'rotate-180': introOpen }"
                        />
                      </button>
                      <Transition
                        enter-active-class="transition-all duration-200 ease-out"
                        leave-active-class="transition-all duration-150 ease-in"
                        enter-from-class="opacity-0 -translate-y-1"
                        leave-to-class="opacity-0 -translate-y-1"
                      >
                        <div
                          v-if="introOpen"
                          class="mt-2 flex flex-col gap-1 text-[11px] leading-relaxed text-white/50"
                        >
                          <div v-for="dim in dimensions" :key="`desc-${dim.key}`">
                            · <span class="text-white/70">{{ $t(`ui.affection.${dim.key}`) }}</span
                            >：{{ $t(`ui.affection.dimDesc.${dim.key}`) }}
                          </div>
                          <div v-for="dim in negDimensions" :key="`neg-desc-${dim.key}`">
                            ·
                            <span class="text-white/70">{{
                              $t(`ui.affection.neg.${dim.key}`)
                            }}</span
                            >：{{ $t(`ui.affection.negDesc.${dim.key}`) }}
                          </div>
                          <div class="mt-1">{{ $t("ui.affection.introEval") }}</div>
                          <div>{{ $t("ui.affection.introPersist") }}</div>
                          <div>{{ $t("ui.affection.introOverflow") }}</div>
                          <div>{{ $t("ui.affection.introNegative") }}</div>
                        </div>
                      </Transition>
                    </div>
                  </div>
                </Transition>
              </div>
            </div>
          </Transition>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ChevronDown, CloudRain } from "lucide-vue-next";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useI18n } from "vue-i18n";
import Button from "../base/widget/Button.vue";
import AffectionRadar from "./AffectionRadar.vue";
import type { RadarPalette } from "./AffectionRadar.vue";
import HeartLiquid from "./HeartLiquid.vue";
import { useGameStore } from "../../stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useSettingsStore } from "@/stores/modules/settings";
import { avatarFolderParams } from "@/composables/role/useRoleAvatar";
import { getAvatarFile } from "@/api/services/character";
import { getAffection } from "@/api/services/affection";
import type { AffectionVector, NegativeVector } from "@/stores/modules/game/state";

const { t } = useI18n();
const gameStore = useGameStore();
const uiStore = useUIStore();
const settingsStore = useSettingsStore();

/** 心跳动画开关（高级设置 → 主菜单，立即生效；关闭后液体爱心静止） */
const heartbeatEnabled = computed(() => settingsStore.affectionHeartbeatEnabled);
/** 液体波浪动画开关（关闭后液面为静止平面，液位弹簧保留） */
const waveEnabled = computed(() => settingsStore.affectionWaveEnabled);

const enabled = ref(false);
const introOpen = ref(false);

function toggleEnabled() {
  enabled.value = !enabled.value;
}
function close() {
  enabled.value = false;
}

const role = computed(() => gameStore.currentInteractRole);
const affection = computed(() => role.value?.affection ?? null);
const negative = computed(() => role.value?.negative ?? null);

type DimKey = keyof AffectionVector;
const dimensions: { key: DimKey }[] = [
  { key: "fondness" },
  { key: "trust" },
  { key: "intimacy" },
  { key: "rapport" },
  { key: "interest" },
  { key: "longing" },
];
type NegDimKey = keyof NegativeVector;
const negDimensions: { key: NegDimKey }[] = [
  { key: "anger" },
  { key: "hurt" },
  { key: "disappointment" },
  { key: "indifference" },
  { key: "jealousy" },
  { key: "estrangement" },
];

// ── 雷达图输入（labels/descs 随界面语言重算；values 数组重建驱动子组件补间） ──
const affectionValues = computed<number[]>(() => {
  const a = affection.value;
  if (!a) return [0, 0, 0, 0, 0, 0];
  return [a.fondness, a.trust, a.intimacy, a.rapport, a.interest, a.longing];
});
const negativeValues = computed<number[]>(() => {
  const n = negative.value;
  if (!n) return [0, 0, 0, 0, 0, 0];
  return [n.anger, n.hurt, n.disappointment, n.indifference, n.jealousy, n.estrangement];
});
const affectionLabels = computed(() => dimensions.map((d) => t(`ui.affection.${d.key}`)));
const affectionDescs = computed(() => dimensions.map((d) => t(`ui.affection.dimDesc.${d.key}`)));
const negativeLabels = computed(() => negDimensions.map((d) => t(`ui.affection.neg.${d.key}`)));
const negativeDescs = computed(() => negDimensions.map((d) => t(`ui.affection.negDesc.${d.key}`)));

/** 好感雷达：粉色系 */
const affectionPalette: RadarPalette = {
  gradientFrom: "#ff5c8a",
  gradientTo: "#ff9ec7",
  stroke: "#ff5c8a",
  vertex: "#ff9ec7",
  vertexOverflow: "#ffd7e8",
  halo: "rgba(255, 92, 138, 0.35)",
  glow: "rgba(255, 92, 138, 0.9)",
  cold: "#7fc4ff",
};
/** 负面雷达：暗红/玫红/暗紫系 */
const negativePalette: RadarPalette = {
  gradientFrom: "#e0416e",
  gradientTo: "#7c5cbf",
  stroke: "#e0416e",
  vertex: "#e88aa8",
  vertexOverflow: "#ffc4d6",
  halo: "rgba(224, 65, 110, 0.4)",
  glow: "rgba(224, 65, 110, 0.9)",
  cold: "#7fc4ff",
};

// ── 平均值与档位 ─────────────────────────────────────
const average = computed(() => {
  const a = affection.value;
  if (!a) return null;
  return Math.round((a.fondness + a.trust + a.intimacy + a.rapport + a.interest + a.longing) / 6);
});

const TIER_ORDER = [
  "estranged",
  "acquainted",
  "plain",
  "familiar",
  "deep",
  "blazing",
  "overflow",
] as const;
type TierKey = (typeof TIER_ORDER)[number];
/** 各档位下限：疏离 <0 / 初识 0-20 / 平淡 21-40 / 熟络 41-60 / 深厚 61-80 / 炽烈 81-100 / 满溢 >100 */
const TIER_FLOOR: Record<TierKey, number> = {
  estranged: Number.NEGATIVE_INFINITY,
  acquainted: 0,
  plain: 21,
  familiar: 41,
  deep: 61,
  blazing: 81,
  overflow: 101,
};
const TIER_COLORS: Record<TierKey, string> = {
  estranged: "#7fc4ff",
  acquainted: "#8ec5ff",
  plain: "#b8c4d6",
  familiar: "#ffd479",
  deep: "#ff9ec7",
  blazing: "#ff5c8a",
  overflow: "#ff3d71",
};

const tier = computed<TierKey | null>(() => {
  const v = average.value;
  if (v === null) return null;
  if (v < 0) return "estranged";
  if (v <= 20) return "acquainted";
  if (v <= 40) return "plain";
  if (v <= 60) return "familiar";
  if (v <= 80) return "deep";
  if (v <= 100) return "blazing";
  return "overflow";
});
const tierColor = computed(() => (tier.value ? TIER_COLORS[tier.value] : "#ff9ec7"));
const tierBadgeStyle = computed(() => {
  const c = tierColor.value;
  return { color: c, borderColor: `${c}80`, background: `${c}1f` };
});

// ── 爱心心跳：心动周期随好感平均值加快（无数据 1.8s → 满溢 0.7s） ──
const heartbeatStyle = computed(() => {
  const v = average.value;
  const duration = v === null ? 1.8 : Math.min(2.2, Math.max(0.7, 1.9 - v / 125));
  return { "--heartbeat-duration": `${duration.toFixed(2)}s` };
});

/** 距下一档的点数与本档内进度（满溢/无数据时为 null） */
const nextTierInfo = computed((): { key: TierKey; points: number; progress: number } | null => {
  const v = average.value;
  const t = tier.value;
  if (v === null || t === null || t === "overflow") return null;
  const nextKey = TIER_ORDER[TIER_ORDER.indexOf(t) + 1];
  const need = TIER_FLOOR[nextKey];
  // 疏离档以当前负值为进度起点，其余档以本档下限为起点
  const start = t === "estranged" ? v : TIER_FLOOR[t];
  const progress = Math.min(100, Math.max(0, ((v - start) / (need - start)) * 100));
  return { key: nextKey, points: Math.max(0, need - v), progress };
});

// ── 当前负面情绪 chips：强度 > 30 的负面维度；峰值 > 60 时档位徽章红色警示 ──
const negativeChips = computed(() => {
  const n = negative.value;
  if (!n) return [];
  return negDimensions
    .filter((d) => n[d.key] > 30)
    .map((d) => ({ key: d.key, value: Math.round(n[d.key]) }));
});
const negativePeak = computed(() => {
  const n = negative.value;
  return n ? Math.max(...Object.values(n)) : 0;
});

// ── 最近一次评估变化（仅展示当前角色的；好感增量 + 负面增量） ──
const recentChange = computed(() => {
  const c = gameStore.lastAffectionChange;
  const r = role.value;
  if (!c || !r || c.roleId !== r.roleId) return null;
  const entries = dimensions
    .filter((d) => typeof c.deltas[d.key] === "number" && c.deltas[d.key] !== 0)
    .map((d) => ({ key: d.key, delta: c.deltas[d.key] }));
  const negEntries = negDimensions
    .filter((d) => typeof c.negativeDeltas[d.key] === "number" && c.negativeDeltas[d.key] !== 0)
    .map((d) => ({ key: d.key, delta: c.negativeDeltas[d.key] }));
  if (entries.length === 0 && negEntries.length === 0 && !c.reason) return null;
  return { entries, negEntries, reason: c.reason };
});

// 头像解析：复用 useRoleAvatar 的归一化纯函数，情绪固定「头像」（getAvatarFile 内部处理）
const avatarUrl = ref("");
let resolveAvatarId = 0;

watch(
  () =>
    role.value
      ? ([role.value.roleId, role.value.character_folder, role.value.clothesName] as const)
      : null,
  async () => {
    const r = role.value;
    if (!r) {
      avatarUrl.value = "";
      return;
    }
    const currentId = ++resolveAvatarId;
    try {
      const { characterFolder, clothesName } = avatarFolderParams(r);
      const path = await getAvatarFile(characterFolder, clothesName);
      if (currentId === resolveAvatarId) avatarUrl.value = convertFileSrc(path);
    } catch {
      if (currentId === resolveAvatarId) avatarUrl.value = "";
    }
  },
  { immediate: true },
);

watch(
  () => uiStore.showSettings,
  (show) => {
    if (show) enabled.value = false;
  },
);

// 展开时若当前角色还没有好感度数据（如 character:switch 临时建档的角色），
// 兜底全量拉一次（init / select_character 数据一般已携带）
watch(enabled, async (v) => {
  if (!v) return;
  if (gameStore.currentInteractRole?.affection) return;
  try {
    const all = await getAffection();
    for (const [roleId, values] of Object.entries(all)) {
      const r = gameStore.gameRoles[Number(roleId)];
      if (r) {
        r.affection = values;
        r.negative = values.negative;
      }
    }
  } catch (e) {
    console.warn("[Affection] 兜底拉取好感度失败:", e);
  }
});
</script>

<style scoped>
/* 爱心心跳：经典「怦-怦」双跳节奏，周期由 --heartbeat-duration 内联控制（随好感加快） */
.affection-heartbeat {
  transform-box: fill-box;
  transform-origin: center;
  animation: affection-heartbeat var(--heartbeat-duration, 1.8s) ease-in-out infinite;
}
@keyframes affection-heartbeat {
  0%,
  100% {
    transform: scale(1);
  }
  14% {
    transform: scale(1.25);
  }
  28% {
    transform: scale(1);
  }
  42% {
    transform: scale(1.16);
  }
  60% {
    transform: scale(1);
  }
}

/* 切换角色：旧内容轻微上浮淡出，新内容自下淡入 */
.affection-role-enter-active,
.affection-role-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}
.affection-role-enter-from {
  opacity: 0;
  transform: translateY(4px);
}
.affection-role-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

/* 档位徽章：pill，颜色由档位内联样式决定；满溢档粉色呼吸光晕，负面峰值高时红色警示环 */
.affection-tier-badge {
  padding: 1px 8px;
  border-radius: 9999px;
  font-size: 11px;
  line-height: 1.5;
  border: 1px solid;
  white-space: nowrap;
}
.affection-tier-pulse {
  animation: affection-tier-glow 1.6s ease-in-out infinite;
}
@keyframes affection-tier-glow {
  0%,
  100% {
    box-shadow: 0 0 2px rgba(255, 61, 113, 0.3);
  }
  50% {
    box-shadow: 0 0 10px rgba(255, 61, 113, 0.7);
  }
}
.affection-tier-alert {
  animation: affection-tier-alert 1.2s ease-in-out infinite;
}
@keyframes affection-tier-alert {
  0%,
  100% {
    box-shadow: 0 0 2px rgba(255, 77, 109, 0.35);
  }
  50% {
    box-shadow: 0 0 12px rgba(255, 77, 109, 0.85);
  }
}

/* 最近变化的增量小芯片：好感正粉负蓝；负面向增暗红、向减青绿（消解） */
.affection-delta-chip {
  padding: 1px 8px;
  border-radius: 9999px;
  font-size: 11px;
  line-height: 1.5;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  white-space: nowrap;
}
.affection-delta-up {
  color: #ff9ec7;
}
.affection-delta-down {
  color: #7fc4ff;
}
.affection-neg-up {
  color: #f06292;
}
.affection-neg-down {
  color: #8fd6a8;
}

/* 「当前负面情绪」芯片：暗红/玫红系，与好感正增量粉色芯片区分 */
.affection-neg-chip {
  padding: 1px 8px;
  border-radius: 9999px;
  font-size: 11px;
  line-height: 1.5;
  color: #f5a8c4;
  background: rgba(224, 80, 126, 0.14);
  border: 1px solid rgba(224, 80, 126, 0.35);
  white-space: nowrap;
}
</style>
