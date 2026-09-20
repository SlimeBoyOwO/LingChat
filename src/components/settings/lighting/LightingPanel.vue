<template>
  <div class="flex flex-col gap-5 p-2">
    <div class="text-xs leading-relaxed text-white/50">
      {{ $t("settings.background.lighting.description") }}
    </div>

    <!-- ========== 总开关 ========== -->
    <Toggle :checked="settings.lighting.masterEnabled" @change="setMaster($event)">
      {{ $t("settings.background.lighting.master") }}
    </Toggle>

    <!-- ========== 当前生效回显 ========== -->
    <!-- 光影会整体改变画面，只靠高亮卡片说明「选了哪个」不够：这里直说现在
         到底是剧本、插件还是面板在控制灯光。 -->
    <div
      class="flex flex-col gap-2 rounded-lg border border-white/10 bg-white/5 px-3 py-2.5"
      :class="{ 'opacity-50': !settings.lighting.masterEnabled }"
    >
      <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
        <span class="text-white/45">{{ $t("settings.background.lighting.current") }}</span>
        <span class="font-bold text-amber-300/90">{{ activeLabel }}</span>
        <span class="text-white/35">·</span>
        <span class="text-white/60">{{ sourceLabel }}</span>
      </div>
      <div v-if="!settings.lighting.masterEnabled" class="text-xs text-white/40">
        {{ $t("settings.background.lighting.masterOffHint") }}
      </div>
      <div v-else-if="runtimeOverride" class="flex flex-wrap items-center gap-x-3 gap-y-2">
        <span class="text-xs text-yellow-300/80">
          {{ $t("settings.background.lighting.overriddenHint", { who: sourceLabel }) }}
        </span>
        <button
          class="rounded-full border border-yellow-500/30 bg-yellow-500/15 px-3 py-1 text-xs
            font-bold text-yellow-200 transition-colors hover:bg-yellow-500/25"
          @click="clearRuntime"
        >
          {{ $t("settings.background.lighting.clearRuntime") }}
        </button>
      </div>
    </div>

    <!-- ========== 全局预设 ========== -->
    <div class="flex flex-col gap-2">
      <label class="text-sm font-medium text-white/70">
        {{ $t("settings.background.lighting.presetsTitle") }}
      </label>
      <div class="text-xs text-white/40">
        {{ $t("settings.background.lighting.presetsHint") }}
      </div>

      <div
        v-if="!lightingStore.presetsLoaded && presets.length === 0"
        class="text-xs text-white/40"
      >
        {{ $t("settings.background.lighting.loading") }}
      </div>

      <div class="grid grid-cols-[repeat(auto-fill,minmax(13rem,1fr))] gap-2">
        <!-- 跟随场景：清空全局预设，交回场景自带的灯光 -->
        <button
          class="rounded-lg border px-3 py-2 text-left transition-all"
          :class="
            settings.lighting.globalPreset
              ? 'border-white/10 bg-white/5 hover:border-white/25'
              : 'border-amber-400/60 bg-amber-400/15 ring-1 ring-amber-400/40'
          "
          :disabled="!settings.lighting.masterEnabled"
          @click="setGlobalPreset('')"
        >
          <div class="text-sm font-bold text-white/90">
            {{ $t("settings.background.lighting.presetFollow") }}
          </div>
          <div class="mt-0.5 text-xs leading-snug text-white/45">
            {{ $t("settings.background.lighting.presetFollowDesc") }}
          </div>
        </button>

        <button
          v-for="p in presets"
          :key="p.id"
          class="rounded-lg border px-3 py-2 text-left transition-all"
          :class="
            settings.lighting.globalPreset === p.id
              ? 'border-amber-400/60 bg-amber-400/15 ring-1 ring-amber-400/40'
              : 'border-white/10 bg-white/5 hover:border-white/25'
          "
          :disabled="!settings.lighting.masterEnabled"
          @click="setGlobalPreset(p.id)"
        >
          <div class="text-sm font-bold text-white/90">{{ p.name }}</div>
          <div class="mt-0.5 line-clamp-2 text-xs leading-snug text-white/45">
            {{ p.description }}
          </div>
          <div class="mt-1 flex flex-wrap gap-1">
            <span
              v-for="m in p.mood.slice(0, 4)"
              :key="m"
              class="rounded-full bg-white/10 px-1.5 py-0.5 text-[10px] text-white/50"
              >{{ m }}</span
            >
          </div>
        </button>
      </div>
    </div>

    <!-- ========== 分项开关 ========== -->
    <div class="flex flex-col gap-2">
      <label class="text-sm font-medium text-white/70">
        {{ $t("settings.background.lighting.switchesTitle") }}
      </label>
      <div class="text-xs text-white/40">
        {{ $t("settings.background.lighting.switchesHint") }}
      </div>
      <div class="flex flex-col gap-2.5">
        <Toggle
          v-for="item in switchItems"
          :key="item.key"
          :checked="settings.lighting[item.key]"
          :disabled="!settings.lighting.masterEnabled"
          @change="setSwitch(item.key, $event)"
        >
          {{ item.label }}
        </Toggle>
      </div>
    </div>

    <!-- ========== 低性能模式 ========== -->
    <div class="flex flex-col gap-2 border-t border-white/10 pt-4">
      <Toggle
        :checked="settings.lighting.lowPerfMode"
        :disabled="!settings.lighting.masterEnabled"
        @change="setSwitch('lowPerfMode', $event)"
      >
        {{ $t("settings.background.lighting.lowPerf") }}
      </Toggle>
      <div class="text-xs text-white/40">
        {{ $t("settings.background.lighting.lowPerfHint") }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { computed, onMounted } from "vue";
  import { useI18n } from "vue-i18n";
  import { Toggle } from "../../base";
  import { useSettingsStore } from "../../../stores/modules/settings";
  import type { LightingSettings } from "../../../stores/modules/settings";
  import { useLightingStore } from "../../../stores/modules/lighting";
  import { useDialogStore } from "../../../stores/modules/ui/dialog";
  import { clearLighting } from "../../../api/services/lighting";

  const settings = useSettingsStore();
  const lightingStore = useLightingStore();
  const dialogStore = useDialogStore();
  const { t } = useI18n();

  const presets = computed(() => lightingStore.presets);
  const runtimeOverride = computed(() => lightingStore.override !== null);

  function setMaster(enabled: boolean) {
    settings.updateLighting({ masterEnabled: enabled });
  }

  /**
   * 手动点预设 = 直接接管灯光。
   *
   * 剧本/AI 留下的运行时覆盖压在全局预设之上，不先清掉就会出现「面板高亮跳到新
   * 预设、画面却还是旧的那盏」；用户以为没生效，之后让 AI 调灯也会被同一份残留
   * 骗过。清完之后靠 `lighting:change` 广播把真实状态回传给后端，不留暗状态。
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

  type SwitchKey = keyof Omit<LightingSettings, "masterEnabled" | "globalPreset">;

  const switchItems = computed(() =>
    (
      [
        ["overlayEnabled", "settings.background.lighting.overlay"],
        ["directionalEnabled", "settings.background.lighting.directional"],
        ["rimEnabled", "settings.background.lighting.rim"],
        ["bloomEnabled", "settings.background.lighting.bloom"],
        ["vignetteEnabled", "settings.background.lighting.vignette"],
        ["gradeEnabled", "settings.background.lighting.grade"],
        ["breathingEnabled", "settings.background.lighting.breathing"],
      ] as const
    ).map(([key, labelKey]) => ({ key: key as SwitchKey, label: t(labelKey) }))
  );

  function setSwitch(key: SwitchKey, value: boolean) {
    settings.updateLighting({ [key]: value } as Partial<LightingSettings>);
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
