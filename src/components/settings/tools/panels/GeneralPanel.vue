<template>
  <div>
    <p class="mb-4 px-1 text-sm text-gray-300">{{ $t("ui.toolCalls.accessModeHint") }}</p>
    <div class="grid gap-3">
      <button
        v-for="mode in accessModes"
        :key="mode"
        type="button"
        class="w-full rounded-xl border px-4 py-3 text-left transition-all duration-200"
        :class="[
          settings.access_mode === mode
            ? mode === 'full_access'
              ? 'border-amber-400/80 bg-amber-400/10 ring-1 ring-amber-400/25'
              : 'border-brand bg-brand/15 ring-brand/25 ring-1'
            : 'border-white/15 bg-white/5 hover:bg-white/10',
        ]"
        @click="selectAccessMode(mode)"
      >
        <div class="flex items-center justify-between gap-3">
          <span class="flex items-center gap-2.5">
            <span
              class="flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition-colors duration-200"
              :class="
                settings.access_mode === mode
                  ? mode === 'full_access'
                    ? 'border-amber-400'
                    : 'border-brand'
                  : 'border-white/30'
              "
            >
              <span
                v-if="settings.access_mode === mode"
                class="h-2 w-2 rounded-full"
                :class="mode === 'full_access' ? 'bg-amber-400' : 'bg-brand'"
              ></span>
            </span>
            <span
              class="font-semibold"
              :class="mode === 'full_access' ? 'text-amber-300' : 'text-white'"
            >
              {{ $t(`ui.toolCalls.accessModes.${mode}.label`) }}
            </span>
          </span>
          <span
            v-if="settings.access_mode === mode"
            class="rounded-full border px-2 py-0.5 text-xs"
            :class="
              mode === 'full_access'
                ? 'border-amber-400/60 text-amber-300'
                : 'border-brand/60 text-brand'
            "
          >
            {{ $t("ui.toolCalls.accessModeSelected") }}
          </span>
        </div>
        <p class="mt-1 text-sm text-gray-300">
          {{ $t(`ui.toolCalls.accessModes.${mode}.description`) }}
        </p>
      </button>
    </div>

    <ToolSettingsCard
      class="mt-3"
      :title="$t('ui.toolCalls.maxToolRoundsTitle')"
      :description="$t('ui.toolCalls.maxToolRoundsHint')"
    >
      <template #action>
        <div class="flex flex-col items-start gap-1 sm:items-end">
          <label
            class="focus-within:border-brand flex items-center gap-2 rounded-lg border border-white/20 bg-black/20 px-3 py-2"
          >
            <input
              v-model.number="settings.max_tool_rounds"
              type="number"
              :min="MIN_TOOL_ROUND_LIMIT"
              :max="MAX_TOOL_ROUND_LIMIT"
              step="1"
              class="w-16 [appearance:textfield] bg-transparent text-center font-semibold text-white outline-none [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
              :aria-label="$t('ui.toolCalls.maxToolRoundsTitle')"
            />
            <span class="text-sm text-gray-300">{{ $t("ui.toolCalls.maxToolRoundsUnit") }}</span>
          </label>
          <p class="px-1 text-xs text-gray-400">
            {{
              $t("ui.toolCalls.maxToolRoundsRange", {
                min: MIN_TOOL_ROUND_LIMIT,
                max: MAX_TOOL_ROUND_LIMIT,
                default: DEFAULT_TOOL_ROUND_LIMIT,
              })
            }}
          </p>
        </div>
      </template>
    </ToolSettingsCard>

    <ToolSettingsCard
      class="mt-3"
      :title="$t('ui.toolCalls.toolRagTitle')"
      :description="$t('ui.toolCalls.toolRagHint')"
    >
      <template #action>
        <div class="flex items-center">
          <Toggle
            :checked="settings.tool_rag"
            @change="(value: boolean) => (settings.tool_rag = value)"
          />
        </div>
      </template>
      <div
        v-if="settings.tool_rag"
        class="mt-3 flex flex-col gap-3 border-t border-white/10 pt-3 sm:flex-row sm:items-center sm:justify-between"
      >
        <div class="min-w-0">
          <h4 class="text-sm font-semibold text-white">
            {{ $t("ui.toolCalls.toolCacheCapacityTitle") }}
          </h4>
          <p class="mt-1 text-xs leading-relaxed text-gray-400">
            {{
              $t("ui.toolCalls.toolCacheCapacityHint", {
                min: MIN_TOOL_CACHE_CAPACITY,
                max: MAX_TOOL_CACHE_CAPACITY,
              })
            }}
          </p>
        </div>
        <label
          class="focus-within:border-brand flex shrink-0 items-center gap-2 rounded-lg border border-white/20 bg-black/20 px-3 py-2"
        >
          <input
            v-model.number="settings.tool_cache_capacity"
            type="number"
            :min="MIN_TOOL_CACHE_CAPACITY"
            :max="MAX_TOOL_CACHE_CAPACITY"
            step="1"
            class="w-14 [appearance:textfield] bg-transparent text-center font-semibold text-white outline-none [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
            :aria-label="$t('ui.toolCalls.toolCacheCapacityTitle')"
          />
          <span class="text-sm text-gray-300">{{ $t("ui.toolCalls.toolCacheCapacityUnit") }}</span>
        </label>
      </div>
    </ToolSettingsCard>

    <ToolSettingsCard
      class="mt-3"
      :title="$t('ui.toolCalls.toolRecommendTitle')"
      :description="$t('ui.toolCalls.toolRecommendHint')"
    >
      <div class="mt-3 flex flex-wrap gap-2">
        <button
          v-for="mode in toolRecommendModes"
          :key="mode"
          type="button"
          class="rounded-lg border px-3 py-1.5 text-sm transition-colors duration-200"
          :class="
            settings.tool_recommend_mode === mode
              ? 'border-brand bg-brand/15 text-white'
              : 'border-white/15 bg-black/20 text-gray-300 hover:bg-white/10'
          "
          @click="settings.tool_recommend_mode = mode"
        >
          {{ $t(`ui.toolCalls.toolRecommendModes.${mode}.label`) }}
        </button>
      </div>
      <p class="mt-2 text-xs leading-relaxed text-gray-400">
        {{ $t(`ui.toolCalls.toolRecommendModes.${settings.tool_recommend_mode}.description`) }}
      </p>
    </ToolSettingsCard>

    <div
      v-if="settings.access_mode === 'full_access'"
      class="mt-3 rounded-lg border-l-4 border-amber-400/70 bg-amber-400/10 px-4 py-3 text-sm text-amber-200/90"
    >
      {{ $t("ui.toolCalls.fullAccessSettingsWarning") }}
    </div>
    <div
      v-if="settings.access_mode === 'full_access' && windows"
      class="mt-3 rounded-xl border border-white/15 bg-black/20 px-4 py-3"
    >
      <div class="flex items-center gap-2 font-semibold text-white">
        <span
          class="h-2.5 w-2.5 rounded-full"
          :class="elevationStatus === 'elevated' ? 'bg-emerald-400' : 'bg-amber-400'"
        ></span>
        {{ $t("ui.toolCalls.adminModeTitle") }}
      </div>
      <p class="mt-2 text-sm text-gray-300">
        {{
          $t(
            elevationStatus === "elevated"
              ? "ui.toolCalls.adminModeElevated"
              : elevationStatus === "checking"
                ? "ui.toolCalls.adminModeChecking"
                : "ui.toolCalls.adminModeStandard",
          )
        }}
      </p>
      <ToolActionButton
        v-if="elevationStatus !== 'elevated'"
        class="mt-3"
        variant="warning"
        :disabled="elevationRestarting || elevationStatus === 'checking'"
        @click="restartAsAdmin"
      >
        {{
          $t(
            elevationRestarting
              ? "ui.toolCalls.adminModeRestarting"
              : "ui.toolCalls.adminModeRestart",
          )
        }}
      </ToolActionButton>
      <p v-if="elevationStatus !== 'elevated'" class="mt-2 text-xs text-gray-400">
        {{ $t("ui.toolCalls.adminModeHint") }}
      </p>
    </div>

    <ToolSettingsCard
      class="mt-6"
      tone="warning"
      :title="$t('ui.toolCalls.resetAll')"
      :description="$t('ui.toolCalls.resetAllHint')"
    >
      <ToolActionButton variant="warning" class="mt-3" @click="resetAll">
        {{ $t("ui.toolCalls.resetAll") }}
      </ToolActionButton>
    </ToolSettingsCard>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  getToolElevationStatus,
  restartToolProcessAsAdmin,
  type ToolAccessMode,
  type ToolRecommendMode,
  type ToolSettings,
} from "@/api/services/tool-settings";
import { useToolSettingsStore } from "@/stores/modules/settings/tools";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import { useUIStore } from "@/stores/modules/ui/ui";
import { isWindows } from "@/utils/platform";
import Toggle from "@/components/base/widget/Toggle.vue";
import ToolActionButton from "@/components/settings/tools/ToolActionButton.vue";
import ToolSettingsCard from "@/components/settings/tools/ToolSettingsCard.vue";

const settings = defineModel<ToolSettings>({ required: true });
const { t } = useI18n();
const store = useToolSettingsStore();
const dialogStore = useDialogStore();

const accessModes: ToolAccessMode[] = ["manual", "auto_approve", "full_access"];
const toolRecommendModes: ToolRecommendMode[] = ["off", "agent", "rag"];
const DEFAULT_TOOL_ROUND_LIMIT = 8;
const MIN_TOOL_ROUND_LIMIT = 1;
const MAX_TOOL_ROUND_LIMIT = 64;
const MIN_TOOL_CACHE_CAPACITY = 1;
const MAX_TOOL_CACHE_CAPACITY = 20;

const windows = isWindows();
const elevationStatus = ref<"checking" | "standard" | "elevated">("checking");
const elevationRestarting = ref(false);

async function selectAccessMode(mode: ToolAccessMode) {
  if (mode === settings.value.access_mode) return;
  if (mode === "full_access") {
    const approved = await dialogStore.confirm(
      t("ui.toolCalls.fullAccessConfirmMessage"),
      t("ui.toolCalls.fullAccessConfirmTitle"),
    );
    if (!approved) return;
  }
  settings.value.access_mode = mode;
}

async function refreshElevationStatus() {
  elevationStatus.value = "checking";
  try {
    elevationStatus.value = (await getToolElevationStatus()) ? "elevated" : "standard";
  } catch (error) {
    console.warn("读取管理员权限状态失败:", error);
    elevationStatus.value = "standard";
  }
}

async function restartAsAdmin() {
  if (elevationRestarting.value) return;
  const approved = await dialogStore.confirm(
    t("ui.toolCalls.adminModeConfirmMessage"),
    t("ui.toolCalls.adminModeConfirmTitle"),
  );
  if (!approved) return;
  await store.saveDraft("general");
  elevationRestarting.value = true;
  try {
    await restartToolProcessAsAdmin();
  } catch (error) {
    elevationRestarting.value = false;
    const ui = useUIStore();
    ui.showNotification({
      type: "error",
      title: t("ui.toolCalls.adminModeRestart"),
      message: t("ui.toolCalls.adminModeRestartFailed", { message: String(error) }),
      skipTipsCheck: true,
    });
  }
}

async function resetAll() {
  const approved = await dialogStore.confirm(
    t("ui.toolCalls.resetConfirmMessage"),
    t("ui.toolCalls.resetConfirmTitle"),
  );
  if (!approved) return;
  store.disableAll();
  store.openDraft("general");
  Object.assign(settings.value, store.drafts.general);
}

onMounted(() => {
  if (windows) void refreshElevationStatus();
});
</script>
