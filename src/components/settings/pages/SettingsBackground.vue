<template>
  <MenuPage>
    <!-- ========== 场景管理（含背景分类、收藏排序、右键移动） ========== -->
    <SceneManageSection />

    <MenuItem :title="$t('settings.background.particle.title')" size="large">
      <template #header>
        <Sparkles :size="20" />
      </template>
      <div class="effect-list flex gap-4 overflow-x-auto pb-2">
        <Button
          v-for="opt in particleOptions"
          :key="opt.value"
          type="transparent"
          :active="currentParticle === opt.value"
          @click="updateParticle(opt.value)"
        >
          <component :is="opt.icon" :size="16" style="margin-right: 6px" />
          {{ opt.label }}
        </Button>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.background.animation.switchTitle')" size="large">
      <template #header>
        <Settings :size="20" />
      </template>
      <div class="flex flex-col gap-3">
        <Toggle
          :checked="mainMenuStarsEnabled"
          @change="settingsStore.setMainMenuStarsEnabled($event)"
        >
          {{ $t("settings.background.animation.mainMenuStars") }}
        </Toggle>
        <Toggle
          :checked="mainMenuMeteorsEnabled"
          @change="settingsStore.setMainMenuMeteorsEnabled($event)"
        >
          {{ $t("settings.background.animation.mainMenuMeteors") }}
        </Toggle>
        <Toggle
          :checked="globalMouseTrailEnabled"
          @change="settingsStore.setGlobalMouseTrailEnabled($event)"
        >
          {{ $t("settings.background.animation.mouseTrail") }}
        </Toggle>
        <Toggle
          :checked="clickAnimationEnabled"
          @change="settingsStore.setClickAnimationEnabled($event)"
        >
          {{ $t("settings.background.animation.clickAnimation") }}
        </Toggle>
        <div class="flex items-center gap-2">
          <Button
            type="big"
            :active="cursorEffectEngine === 'ba-click-fx'"
            @click="settingsStore.setCursorEffectEngine('ba-click-fx')"
          >
            {{ $t("settings.background.animation.cursorEngine.ba") }}
          </Button>
          <Button
            type="big"
            :active="cursorEffectEngine === 'legacy'"
            @click="settingsStore.setCursorEffectEngine('legacy')"
          >
            {{ $t("settings.background.animation.cursorEngine.legacy") }}
          </Button>
        </div>
        <Toggle
          :checked="sceneAwarenessEnabled"
          @change="settingsStore.setSceneAwarenessEnabled($event)"
        >
          {{ $t("settings.background.animation.sceneAwareness") }}
        </Toggle>
      </div>
    </MenuItem>

    <!-- HDR 模式（仅 Windows：WebView2 强制色彩配置在 HDR 下会发灰/发暗） -->
    <MenuItem v-if="isWindows()" :title="$t('settings.background.hdr.title')" size="large">
      <template #header>
        <Settings :size="20" />
      </template>
      <div class="flex flex-col gap-3">
        <Toggle :checked="hdrModeEnabled" @change="settingsStore.setHdrModeEnabled($event)">
          {{ $t("settings.background.hdr.enable") }}
        </Toggle>
        <p class="text-xs text-yellow-400/70">
          {{ $t("settings.background.hdr.restartHint") }}
        </p>
        <button
          class="self-start rounded-lg border border-amber-500/30 bg-amber-500/20 px-4 py-2 text-sm font-medium text-amber-300 transition-colors hover:bg-amber-500/30 disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="!hdrChanged"
          @click="restartApp"
        >
          {{ $t("settings.background.hdr.restartBtn") }}
        </button>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.background.animation.settingsTitle')" size="large">
      <template #header>
        <Sparkles :size="20" />
      </template>
      <div class="flex flex-col gap-4 p-2">
        <div
          class="flex items-center gap-4 max-[640px]:flex-col max-[640px]:items-stretch max-[640px]:gap-2"
        >
          <span class="min-w-30 text-sm font-medium text-white/90">{{
            $t("settings.background.animation.meteorFps")
          }}</span>
          <Slider
            v-model="meteorFps"
            :min="10"
            :max="60"
            :step="5"
            accent-color="#8b5cf6"
            @change="handleMeteorFpsChange"
            class="flex-1"
          >
            <template #left>{{ meteorFps }} FPS</template>
          </Slider>
          <input
            type="number"
            v-model.number="meteorFpsInput"
            @blur="handleInputBlur"
            @keyup.enter="handleInputEnter"
            min="10"
            max="300"
            class="w-20 rounded-lg border border-white/20 bg-white/10 px-3 py-1.5 text-sm font-medium text-white transition-all focus:border-transparent focus:ring-2 focus:ring-purple-500 focus:outline-none"
          />
        </div>

        <div
          class="flex items-center gap-4 max-[640px]:flex-col max-[640px]:items-stretch max-[640px]:gap-2"
        >
          <span class="min-w-30 text-sm font-medium text-white/90">{{
            $t("settings.background.animation.starsFps")
          }}</span>
          <Slider
            v-model="starsFps"
            :min="10"
            :max="60"
            :step="5"
            accent-color="#fbbf24"
            @change="handleStarsFpsChange"
            class="flex-1"
          >
            <template #left>{{ starsFps }} FPS</template>
          </Slider>
          <input
            type="number"
            v-model.number="starsFpsInput"
            @blur="handleStarsInputBlur"
            @keyup.enter="handleStarsInputEnter"
            min="10"
            max="300"
            class="w-20 rounded-lg border border-white/20 bg-white/10 px-3 py-1.5 text-sm font-medium text-white transition-all focus:border-transparent focus:ring-2 focus:ring-yellow-500 focus:outline-none"
          />
        </div>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.background.perf.title')" size="large">
      <template #header>
        <Cpu :size="20" />
      </template>
      <div class="flex flex-col gap-3">
        <!-- 加载中 -->
        <div v-if="perfLoading" class="flex items-center gap-2 text-sm text-white/60">
          <span
            class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-white/30 border-t-white/80"
          ></span>
          {{ $t("settings.background.perf.detecting") }}
        </div>

        <!-- 检测结果 -->
        <div v-else-if="cpuInfo" class="flex flex-col gap-2">
          <!-- 未知 CPU 提示 -->
          <div
            v-if="cpuInfo.is_unknown && cpuInfo.unknown_message"
            class="flex items-center gap-2 rounded-lg border border-yellow-500/30 bg-yellow-500/15 px-3 py-2 text-sm text-yellow-200"
          >
            <span>⚠️ {{ cpuInfo.unknown_message }}</span>
          </div>

          <!-- GPU 分级不适用 / 未检测到 GPU 提示 -->
          <div
            v-if="gpuInfo?.message"
            class="flex items-center gap-2 rounded-lg border border-yellow-500/30 bg-yellow-500/15 px-3 py-2 text-sm text-yellow-200"
          >
            <span>⚠️ {{ gpuInfo.message }}</span>
          </div>

          <!-- CPU 行 -->
          <div class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.cpuName")
            }}</span>
            <span class="flex-1 font-mono text-sm break-all text-white/90">{{
              cpuInfo.brand
            }}</span>
            <span
              class="shrink-0 rounded-full px-2.5 py-0.5 text-xs font-bold"
              :class="tierBadgeClassFor(cpuInfo.tier as PerfTier)"
              :style="{ backgroundColor: getPerfTierColor(cpuInfo.tier as PerfTier) + '99' }"
            >
              {{ getTierLabel(cpuInfo.tier as PerfTier) }}
            </span>
          </div>

          <!-- GPU 行（分级适用且有检测到 GPU 时显示） -->
          <div v-if="gpuInfo?.is_applicable && gpuInfo.name" class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.gpuName")
            }}</span>
            <span class="flex-1 font-mono text-sm break-all text-white/90">{{ gpuInfo.name }}</span>
            <span
              class="shrink-0 rounded-full px-2.5 py-0.5 text-xs font-bold"
              :class="tierBadgeClassFor(gpuInfo.tier as PerfTier)"
              :style="{ backgroundColor: getPerfTierColor(gpuInfo.tier as PerfTier) + '99' }"
            >
              {{ getTierLabel(gpuInfo.tier as PerfTier) }}
            </span>
          </div>

          <!-- 当前实际调用的 GPU（WebGL 渲染器，反映程序真正在用的卡） -->
          <div v-if="activeGpu?.is_applicable && activeGpu.name" class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.activeGpuName")
            }}</span>
            <span class="flex-1 font-mono text-sm break-all text-white/90">{{
              activeGpu.name
            }}</span>
            <span
              class="shrink-0 rounded-full px-2.5 py-0.5 text-xs font-bold"
              :class="tierBadgeClassFor(activeGpu.tier as PerfTier)"
              :style="{ backgroundColor: getPerfTierColor(activeGpu.tier as PerfTier) + '99' }"
            >
              {{ getTierLabel(activeGpu.tier as PerfTier) }}
            </span>
          </div>

          <!-- 综合等级（取最低） -->
          <div class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.combinedTier")
            }}</span>
            <span
              v-if="combinedTier"
              class="rounded-full px-2.5 py-0.5 text-xs font-bold"
              :class="tierBadgeClassFor(combinedTier)"
              :style="{ backgroundColor: getPerfTierColor(combinedTier) + '99' }"
            >
              {{ getTierLabel(combinedTier) }}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <span class="min-w-16 shrink-0 text-xs font-medium text-white/50">{{
              $t("settings.background.perf.suggestedFps")
            }}</span>
            <span class="text-sm text-white/70">{{ suggestedFps }} FPS</span>
          </div>
        </div>

        <!-- 错误状态 -->
        <div v-else-if="perfError" class="text-sm text-red-300">
          {{ perfError }}
        </div>

        <!-- 重新检测按钮（同时重新检测 CPU 与 GPU） -->
        <button
          class="bg-brand/80 border-brand hover:bg-brand mt-1 self-start rounded-full border px-4 py-1.5 text-sm font-bold text-white shadow-lg shadow-indigo-500/20 transition-all disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="perfLoading"
          @click="handleRedetectPerf"
        >
          {{
            perfLoading
              ? $t("settings.background.perf.detectingShort")
              : $t("settings.background.perf.redetect")
          }}
        </button>
      </div>
    </MenuItem>

    <!-- ========== 对话框外观（自定义） ========== -->
    <MenuItem :title="$t('settings.background.dialog.title')" size="large">
      <template #header>
        <MessageSquare :size="20" />
      </template>
      <DialogAppearancePanel />
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, watch, computed, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { MenuPage, MenuItem } from "../../ui";
import { Button, Toggle, Slider } from "../../base";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { useDialogStore } from "../../../stores/modules/ui/dialog";
import { useSettingsStore } from "../../../stores/modules/settings";
import { isWindows } from "@/utils/platform";
import { relaunch } from "@tauri-apps/plugin-process";
import { Ban, Sparkles, CloudRain, Flower2, Snowflake, PartyPopper, Sun } from "lucide-vue-next";
import { PARTICLE_EFFECTS } from "@/components/game/standard/particles";
import {
  getCpuInfo,
  redetectCpu,
  getTierLabel,
  getSuggestedMaxFps,
  getPerfTierColor,
  getCombinedTier,
  type CpuInfo,
  type PerfTier,
} from "../../../api/services/cpu-perf";
import {
  getGpuInfo,
  redetectGpu,
  getActiveGpu,
  type GpuInfo,
} from "../../../api/services/gpu-perf";
import { Settings, Cpu } from "lucide-vue-next";
import SceneManageSection from "../background/SceneManageSection.vue";
import DialogAppearancePanel from "../dialog/DialogAppearancePanel.vue";

const uiStore = useUIStore();
const settingsStore = useSettingsStore();
const dialogStore = useDialogStore();
const { t } = useI18n();

// 粒子选项来自注册表，与桌宠设置页读同一份真相。
// 从前这里是六个硬编码按钮：星辉（BA）并入主界面可选项时只接了渲染分支，
// 没补进这份列表，于是设置页选不到它。改成读注册表后就不会再漏。
const PARTICLE_ICONS: Record<string, Component> = {
  StarField: Sparkles,
  Rain: CloudRain,
  Sakura: Flower2,
  Snow: Snowflake,
  Fireworks: PartyPopper,
  BA: Sun,
};

const particleOptions = computed(() => [
  { label: t("settings.background.particle.none"), value: "None", icon: Ban },
  ...PARTICLE_EFFECTS.map((p) => ({
    label: t(`settings.background.particle.${p.i18n}`),
    value: p.key,
    icon: PARTICLE_ICONS[p.key] ?? Sparkles,
  })),
]);

const mainMenuStarsEnabled = computed(() => settingsStore.mainMenuStarsEnabled);
const mainMenuMeteorsEnabled = computed(() => settingsStore.mainMenuMeteorsEnabled);
const globalMouseTrailEnabled = computed(() => settingsStore.globalMouseTrailEnabled);
const clickAnimationEnabled = computed(() => settingsStore.clickAnimationEnabled);
const cursorEffectEngine = computed(() => settingsStore.cursorEffectEngine);
const sceneAwarenessEnabled = computed(() => settingsStore.sceneAwarenessEnabled);
const hdrModeEnabled = computed(() => settingsStore.hdrModeEnabled);
const currentParticle = computed(() => settingsStore.backgroundEffect);

// 记录进入设置页时的初始值；开关改变后「立即重启」按钮才可用，改回原值则恢复置灰
const initialHdrMode = ref(settingsStore.hdrModeEnabled);
const hdrChanged = computed(() => settingsStore.hdrModeEnabled !== initialHdrMode.value);

// 立即重启应用（HDR 模式等设置需重启后生效）
async function restartApp() {
  const ok = await dialogStore.confirm(t("settings.background.hdr.restartConfirm"));
  if (!ok) return;
  try {
    await relaunch();
  } catch (e) {
    console.error("重启失败:", e);
    dialogStore.alert(t("settings.background.hdr.restartFailed"));
  }
}
const meteorFps = computed({
  get: () => settingsStore.meteorFps,
  set: (value: number) => {
    const clampedValue = Math.max(10, Math.min(60, value));
    settingsStore.setMeteorFps(clampedValue);
  },
});
const meteorFpsInput = ref(settingsStore.meteorFps);

const starsFps = computed({
  get: () => settingsStore.starsFps,
  set: (value: number) => {
    const clampedValue = Math.max(10, Math.min(60, value));
    settingsStore.setStarsFps(clampedValue);
  },
});
const starsFpsInput = ref(settingsStore.starsFps);

// ── 硬件性能检测（CPU + GPU） ──
const cpuInfo = ref<CpuInfo | null>(null);
const gpuInfo = ref<GpuInfo | null>(null);
const activeGpu = ref<GpuInfo | null>(null);
const perfLoading = ref(true);
const perfError = ref<string | null>(null);

/** 性能等级徽章样式 */
function tierBadgeClassFor(tier: PerfTier): string {
  switch (tier) {
    case "Internet":
      return "bg-gray-500/60 text-gray-100";
    case "Low":
      return "bg-yellow-600/60 text-yellow-100";
    case "Medium":
      return "bg-blue-500/60 text-blue-100";
    case "High":
      return "bg-green-500/60 text-green-100";
    default:
      return "bg-white/20 text-white/60";
  }
}

/** 综合性能等级（取最低；GPU 分级不适用时仅按 CPU）。GPU 取「当前调用」优先，回退「最高性能」。 */
const combinedTier = computed<PerfTier | null>(() => {
  if (!cpuInfo.value) return null;
  const cpuTier = cpuInfo.value.tier as PerfTier;
  const activeTier =
    activeGpu.value?.is_applicable && activeGpu.value.name
      ? (activeGpu.value.tier as PerfTier)
      : null;
  const maxTier = gpuInfo.value?.is_applicable ? (gpuInfo.value.tier as PerfTier) : null;
  const gpuTier = activeTier ?? maxTier;
  return getCombinedTier(cpuTier, gpuTier);
});

const suggestedFps = computed(() =>
  combinedTier.value ? getSuggestedMaxFps(combinedTier.value) : 30,
);

onMounted(async () => {
  // 加载 CPU + GPU 性能信息
  await fetchPerfInfo();
});

async function fetchPerfInfo(): Promise<void> {
  perfLoading.value = true;
  perfError.value = null;
  try {
    // activeGpu 读取真实 WebGL 渲染器（反映当前实际调用的 GPU），失败不影响其余信息
    const [cpu, gpu, active] = await Promise.all([
      getCpuInfo(),
      getGpuInfo(),
      getActiveGpu().catch(() => null),
    ]);
    cpuInfo.value = cpu;
    gpuInfo.value = gpu;
    activeGpu.value = active;
  } catch (e: any) {
    perfError.value = e?.message || t("settings.background.perf.fetchFailed");
    console.error("获取硬件性能信息失败", e);
  } finally {
    perfLoading.value = false;
  }
}

async function handleRedetectPerf(): Promise<void> {
  perfLoading.value = true;
  perfError.value = null;
  try {
    const [cpu, gpu, active] = await Promise.all([
      redetectCpu(),
      redetectGpu(),
      getActiveGpu().catch(() => null),
    ]);
    cpuInfo.value = cpu;
    gpuInfo.value = gpu;
    activeGpu.value = active;
    uiStore.showSuccess({
      title: t("settings.background.perf.detectComplete"),
      message: t("settings.background.perf.tierMessage", {
        tier: combinedTier.value
          ? getTierLabel(combinedTier.value)
          : t("settings.background.perf.unknown"),
      }),
      duration: 3000,
    });
  } catch (e: any) {
    perfError.value = e?.message || t("settings.background.perf.redetectFailed");
    console.error("重新检测硬件性能失败", e);
  } finally {
    perfLoading.value = false;
  }
}

function updateParticle(value: string): void {
  uiStore.setBackgroundEffect(value);
}

function handleMeteorFpsChange(value: number) {
  const clampedValue = Math.max(10, Math.min(60, value));
  meteorFpsInput.value = clampedValue;
  settingsStore.setMeteorFps(clampedValue);
}

function handleInputBlur() {
  let value = Number(meteorFpsInput.value);
  if (isNaN(value) || value < 10) value = 10;
  else if (value > 300) value = 300;
  meteorFpsInput.value = value;
  settingsStore.setMeteorFps(value);
}

function handleInputEnter() {
  handleInputBlur();
}

watch(meteorFps, (newValue) => {
  meteorFpsInput.value = newValue;
});

function handleStarsFpsChange(value: number) {
  const clampedValue = Math.max(10, Math.min(60, value));
  starsFpsInput.value = clampedValue;
  settingsStore.setStarsFps(clampedValue);
}

function handleStarsInputBlur() {
  let value = Number(starsFpsInput.value);
  if (isNaN(value) || value < 10) value = 10;
  else if (value > 300) value = 300;
  starsFpsInput.value = value;
  settingsStore.setStarsFps(value);
}

function handleStarsInputEnter() {
  handleStarsInputBlur();
}

watch(starsFps, (newValue) => {
  starsFpsInput.value = newValue;
});
</script>
