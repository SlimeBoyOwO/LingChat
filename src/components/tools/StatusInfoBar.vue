<template>
  <!--
    顶部状态栏：时间 / 天气。仅自由对话模式显示，与左上角 FreeModeTools、
    右上角菜单按钮同高同风格（icon + 文字），纯展示不接收点击。
  -->
  <div v-if="visible" class="pointer-events-none hidden items-center lg:flex">
    <!-- 两侧装饰：与文字同色的渐隐横线 -->
    <div class="h-px w-10 bg-linear-to-r from-transparent to-white/80 xl:w-16"></div>

    <div
      v-for="item in items"
      :key="item.key"
      class="mx-[5px] flex items-center gap-2 px-[15px] py-[10px] text-lg font-bold text-white [text-shadow:0_2px_4px_rgba(0,0,0,0.2)]"
    >
      <component :is="item.icon" :size="20" :stroke-width="2.5" class="shrink-0" />
      <span>{{ item.text }}</span>
    </div>

    <div class="h-px w-10 bg-linear-to-l from-transparent to-white/80 xl:w-16"></div>
  </div>
</template>

<script setup lang="ts">
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { WEATHER_EFFECTS } from "@/components/game/standard/particles";
import {
  Ban,
  Clock,
  CloudDrizzle,
  CloudFog,
  CloudLightning,
  CloudRain,
  CloudSnow,
  Snowflake,
} from "@lucide/vue";
import { computed, onMounted, onUnmounted, ref, type Component } from "vue";
import { useI18n } from "vue-i18n";

const gameStore = useGameStore();
const uiStore = useUIStore();
const { t } = useI18n();

// 仅在自由对话模式显示（与 FreeModeTools 同一判据：跑剧本时不出现）
const visible = computed(
  () => !uiStore.showSettings && !(gameStore.runningScript && gameStore.runningScript.isRunning),
);

// ===== 时间：每秒走一次，显示到时:分:秒 =====
const now = ref(new Date());
let timer: number | undefined;

onMounted(() => {
  timer = window.setInterval(() => {
    now.value = new Date();
  }, 1000);
});

onUnmounted(() => {
  if (timer !== undefined) window.clearInterval(timer);
});

const timeText = computed(() => {
  const date = now.value;
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");
  return `${hours}:${minutes}:${seconds}`;
});

// ===== 天气：特效 key → 图标（与设置页 SettingsBackground 的映射一致），文案复用设置页词条 =====
const WEATHER_ICONS: Record<string, Component> = {
  Drizzle: CloudDrizzle,
  Rain: CloudRain,
  Thunderstorm: CloudLightning,
  Snow: Snowflake,
  Blizzard: CloudSnow,
  Fog: CloudFog,
};

const weatherKey = computed(() => uiStore.currentWeatherEffect);

// 没开任何天气特效时落回「无」（设置页里「无」用的也是 Ban 图标）
const weatherEffect = computed(() => WEATHER_EFFECTS.find((p) => p.key === weatherKey.value));

const weatherText = computed(() =>
  weatherEffect.value
    ? t(`settings.background.weather.${weatherEffect.value.i18n}`)
    : t("settings.background.weather.none"),
);

const weatherIcon = computed(() =>
  weatherEffect.value ? (WEATHER_ICONS[weatherKey.value] ?? CloudRain) : Ban,
);

const items = computed(() => [
  { key: "time", icon: Clock, text: timeText.value },
  { key: "weather", icon: weatherIcon.value, text: weatherText.value },
]);
</script>
