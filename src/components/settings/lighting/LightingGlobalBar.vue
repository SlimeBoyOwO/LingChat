<template>
  <div class="mb-4 flex flex-col gap-2.5 rounded-xl border border-white/10 bg-white/5 px-3 py-2.5">
    <!-- ========== 上排：两个开关 + 默认光影 ========== -->
    <div class="flex flex-wrap items-center gap-x-4 gap-y-2">
      <div class="w-[13.5rem] shrink-0">
        <Toggle :checked="settings.lighting.masterEnabled" @change="setMaster($event)">
          {{ $t("settings.background.lighting.master") }}
        </Toggle>
      </div>

      <!-- 说明收进悬停提示：这一条常年开着，占一行文字太浪费。 -->
      <div class="w-[10rem] shrink-0" :title="$t('settings.background.lighting.lowPerfHint')">
        <Toggle
          :checked="settings.lighting.lowPerfMode"
          :disabled="!settings.lighting.masterEnabled"
          @change="setLowPerf($event)"
        >
          {{ $t("settings.background.lighting.lowPerf") }}
        </Toggle>
      </div>

      <div class="flex min-w-[17rem] flex-1 items-center gap-2">
        <span class="shrink-0 text-xs text-white/60">
          {{ $t("settings.background.lighting.defaultTitle") }}
        </span>
        <div
          class="min-w-0 flex-1"
          :class="{ 'pointer-events-none opacity-50': !settings.lighting.masterEnabled }"
        >
          <LightingPresetSelect
            :current-label="currentDefaultName"
            :none-label="$t('settings.background.lighting.select.noneDefault')"
            :selected-id="settings.lighting.globalPreset"
            @picked="onPick"
            @saved="onSaved"
          />
        </div>
      </div>
    </div>

    <!-- ========== 下排：当前生效回显 ========== -->
    <!-- 光影会整体改变画面，只靠下拉高亮说明「选了哪个」不够：这里直说现在到底是
         场景自己的灯、默认那盏，还是剧本/插件在临时控制灯光。 -->
    <div class="flex flex-wrap items-center gap-x-2 gap-y-1.5 text-xs">
      <span class="text-white/45">{{ $t("settings.background.lighting.current") }}</span>
      <span class="font-bold text-amber-300/90">{{ activeLabel }}</span>
      <template v-if="settings.lighting.masterEnabled">
        <span class="text-white/35">·</span>
        <span class="text-white/60">{{ sourceLabel }}</span>
      </template>

      <span v-if="!settings.lighting.masterEnabled" class="text-white/40">
        {{ $t("settings.background.lighting.masterOffHint") }}
      </span>
      <div v-else-if="runtimeOverride" class="flex flex-wrap items-center gap-x-3 gap-y-2">
        <span class="text-yellow-300/80">
          {{ $t("settings.background.lighting.overriddenHint", { who: sourceLabel }) }}
        </span>
        <button
          class="rounded-full border border-yellow-500/30 bg-yellow-500/15 px-3 py-1 text-xs font-bold text-yellow-200 transition-colors hover:bg-yellow-500/25"
          @click="clearRuntime"
        >
          {{ $t("settings.background.lighting.clearRuntime") }}
        </button>
      </div>
      <span v-else class="ml-auto min-w-[16rem] flex-1 text-white/35">
        {{ $t("settings.background.lighting.defaultHint") }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 全局光影设置条，嵌在「场景及光影设置」的场景卡片上方。
 *
 * 这里只管跨场景的四件事：总开关、低性能模式、默认光影（兜底那盏）、当前生效回显。
 * 调参和预设管理都在「更新场景」弹窗里 —— 生效优先级是场景灯优先，面板再留一套
 * 完整编辑器就是两处调一处生效，迟早算不到一起。
 */
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { Toggle } from "../../base";
import LightingPresetSelect from "./LightingPresetSelect.vue";
import { useSettingsStore } from "../../../stores/modules/settings";
import { useLightingStore } from "../../../stores/modules/lighting";
import type { LightingPreset } from "../../../api/services/lighting";
import { useDialogStore } from "../../../stores/modules/ui/dialog";
import { clearLighting } from "../../../api/services/lighting";

const settings = useSettingsStore();
const lightingStore = useLightingStore();
const dialogStore = useDialogStore();
const { t } = useI18n();

const presets = computed(() => lightingStore.presets);
const runtimeOverride = computed(() => lightingStore.override !== null);

const currentDefaultName = computed(
  () => presets.value.find((p) => p.id === settings.lighting.globalPreset)?.name ?? "",
);

function setMaster(enabled: boolean) {
  settings.updateLighting({ masterEnabled: enabled });
}

function setLowPerf(enabled: boolean) {
  settings.updateLighting({ lowPerfMode: enabled });
}

/**
 * 手动点预设 = 直接接管灯光。
 *
 * 剧本/AI 留下的运行时覆盖压在所有灯光之上，不先清掉就会出现「下拉已经换成了
 * 新的默认光影、画面却还是临时那盏」；用户以为没生效，之后让 AI 调灯也会被同一
 * 份残留骗过。清完之后靠 `lighting:change` 广播把真实状态回传给后端，不留暗状态。
 */
async function setGlobalPreset(id: string) {
  settings.updateLighting({ globalPreset: id });
  if (!lightingStore.override) return;
  try {
    await clearLighting();
    dialogStore.alert(t("settings.background.lighting.takenOver"));
  } catch (e) {
    console.error("[Lighting] 接管运行时灯光失败:", e);
    dialogStore.alert(t("settings.background.lighting.clearRuntimeFailed"));
  }
}

function onPick(preset: LightingPreset | null) {
  void setGlobalPreset(preset?.id ?? "");
}

/** 下拉在抛出 saved 之前已经把预览用的临时灯光清掉了，这里只负责套用与回执。 */
async function onSaved(preset: LightingPreset) {
  await setGlobalPreset(preset.id);
  dialogStore.alert(t("settings.background.lighting.custom.savedOk", { name: preset.name }));
}

const activeLabel = computed(() => {
  if (!settings.lighting.masterEnabled) return t("settings.background.lighting.masterShort");
  const id = lightingStore.activePresetId;
  if (id) return presets.value.find((p) => p.id === id)?.name ?? id;
  return lightingStore.override
    ? t("settings.background.lighting.presetInline")
    : t("settings.background.lighting.presetFollow");
});

const SOURCE_KEYS: Record<string, string> = {
  scene: "settings.background.lighting.sourceScene",
  global: "settings.background.lighting.sourceGlobal",
  script: "settings.background.lighting.sourceScript",
  tool: "settings.background.lighting.sourceTool",
  panel: "settings.background.lighting.sourcePanel",
};

const sourceLabel = computed(() => {
  const key = SOURCE_KEYS[lightingStore.activeSource];
  return key ? t(key) : lightingStore.activeSource;
});

async function clearRuntime() {
  try {
    await clearLighting();
  } catch (e) {
    console.error("[Lighting] 取消运行时灯光失败:", e);
    dialogStore.alert(t("settings.background.lighting.clearRuntimeFailed"));
  }
}

onMounted(() => {
  void lightingStore.ensurePresets();
});
</script>
