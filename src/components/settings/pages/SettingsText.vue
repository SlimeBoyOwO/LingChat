<template>
  <div class="settings-text-container">
    <MenuPage>
      <MenuItem :title="$t('settings.text.font.title')">
        <template #header>
          <Type :size="20" />
        </template>
        <div class="flex w-full items-stretch gap-2">
          <select
            v-model="fontFamily"
            class="max-w-48 min-w-32 flex-none cursor-pointer appearance-none rounded-lg border
              border-white/20 bg-white/10 py-2 pr-8 pl-3 text-sm text-white transition-colors
              outline-none focus:border-(--accent-color)"
            @change="onFontChange"
            :title="$t('settings.text.font.selectHint')"
          >
            <option value="">{{ $t("settings.text.font.default") }}</option>
            <optgroup v-if="importedFonts.length > 0" :label="$t('settings.text.font.imported')">
              <option
                v-for="f in importedFonts"
                :key="f.name"
                :value="f.name"
                :style="{ fontFamily: `'${f.name}'` }"
              >
                {{ f.name }}
              </option>
            </optgroup>
            <option v-if="importedFonts.length > 0" disabled>──────────</option>
            <option v-if="fontsLoading" value="" disabled>
              {{ $t("settings.text.font.loading") }}
            </option>
            <option v-for="f in systemFonts" :key="f" :value="f" :style="{ fontFamily: `'${f}'` }">
              {{ f }}
            </option>
          </select>
          <button
            class="flex flex-none cursor-pointer items-center justify-center rounded-lg border
              border-white/20 bg-white/10 px-[0.6rem] py-[0.35rem] text-white transition-colors
              hover:border-(--accent-color) hover:bg-white/20 active:bg-white/30"
            @click="handleImportFont"
            :title="$t('settings.text.font.importTitle')"
          >
            <Import :size="18" />
          </button>
          <div class="font-demo" :style="{ fontFamily: demoFontFamily }">
            {{ $t("settings.text.font.demo") }}
          </div>
        </div>
      </MenuItem>

      <MenuItem :title="$t('settings.text.speed.title')">
        <template #header>
          <Zap :size="20" />
        </template>
        <Slider @change="textSpeedChange" v-model="textSpeed">{{
          $t("settings.text.speed.label")
        }}</Slider>
      </MenuItem>

      <MenuItem :title="$t('settings.text.autoAdvance.title')">
        <template #header>
          <Timer :size="20" />
        </template>
        <Slider v-model="autoAdvanceDelay" :min="0" :max="2000" :step="100">
          <template #left>{{ autoAdvanceDelay }}</template>
          <template #right>ms</template>
        </Slider>
      </MenuItem>

      <MenuItem :title="$t('settings.text.sample.title')">
        <template #header>
          <ClipboardList :size="20" />
        </template>
        <Text :speed="textSpeedSample">{{ $t("settings.text.sample.demo") }}</Text>
      </MenuItem>

      <MenuItem :title="$t('settings.text.inlineMotion.title')" size="small">
        <template #header>
          <AlignJustify :size="20" />
        </template>
        <Toggle :checked="settingsStore.text.inlineMotionText" @change="toggleInlineMotionText">
          {{ $t("settings.text.inlineMotion.desc") }}
        </Toggle>
      </MenuItem>

      <!-- 台词合并阈值：同角色连续短句自动合并续打（仅内联动作文本模式生效）；0=关闭 -->
      <MenuItem :title="$t('settings.text.mergeLine.title')" size="small">
        <template #header>
          <AlignJustify :size="20" />
        </template>
        <Slider v-model="mergeLineThreshold" :min="0" :max="100" :step="1">
          <template #left>{{ mergeLineThreshold }}</template>
          <template #right>{{ $t("settings.text.mergeLine.unit") }}</template>
        </Slider>
      </MenuItem>

      <!-- 台词合并续打延迟：上一句展示完成到接续打下一句之间的停顿 -->
      <MenuItem :title="$t('settings.text.mergeDelay.title')" size="small">
        <template #header>
          <Timer :size="20" />
        </template>
        <Slider v-model="mergeLineDelay" :min="0" :max="1000" :step="50">
          <template #left>{{ mergeLineDelay }}</template>
          <template #right>ms</template>
        </Slider>
      </MenuItem>

      <!-- 动作文本合并方式：append=接在后面显示（| 分隔）/ replace=清空旧动作，独立显示本次动作 -->
      <MenuItem :title="$t('settings.text.mergeMotion.title')" size="small">
        <template #header>
          <Combine :size="20" />
        </template>
        <div class="flex items-center gap-2">
          <Button
            type="big"
            :active="mergeMotionMode === 'append'"
            @click="setMergeMotionMode('append')"
          >
            {{ $t("settings.text.mergeMotion.append") }}
          </Button>
          <Button
            type="big"
            :active="mergeMotionMode === 'replace'"
            @click="setMergeMotionMode('replace')"
          >
            {{ $t("settings.text.mergeMotion.replace") }}
          </Button>
        </div>
      </MenuItem>

      <MenuItem :title="$t('settings.text.sedentary.title')" size="small">
        <template #header>
          <GlassWater :size="20" />
        </template>
        <Toggle :checked="settingsStore.text.sedentaryReminder" @change="toggleSedentaryReminder">
          {{ $t("settings.text.sedentary.desc") }}
        </Toggle>
      </MenuItem>

      <!-- Vue DevTools 显示开关：仅开发模式（vite-plugin-vue-devtools 只在 serve 注入），
           全局所有窗口（主窗 / 投屏 / 日志 / 桌宠）都生效 -->
      <MenuItem v-if="isDevMode" :title="$t('settings.text.vueDevTools.title')" size="small">
        <template #header>
          <Bug :size="20" />
        </template>
        <Toggle :checked="settingsStore.text.vueDevToolsEnabled" @change="toggleVueDevTools">
          {{ $t("settings.text.vueDevTools.desc") }}
        </Toggle>
      </MenuItem>

      <MenuItem :title="$t('settings.text.memory.title')" size="small">
        <div v-for="setting in envSettings" :key="setting.key" class="">
          <!-- 使用 SettingItem 组件渲染不同类型的输入控件 -->
          <Toggle
            :checked="setting.value.toLowerCase() === 'true'"
            @change="handleMemorySettingChange($event, setting)"
          >
            {{ $t("settings.text.memory.desc") }}
          </Toggle>
        </div>
        <template #header>
          <Star :size="20" />
        </template>
      </MenuItem>

      <MenuItem :title="$t('settings.text.voiceSound.title')" size="small">
        <template #header>
          <Earth :size="20" />
        </template>
        <Toggle @change="voiceSound">{{ $t("settings.text.voiceSound.desc") }}</Toggle>
      </MenuItem>

      <MenuItem :title="$t('settings.text.engineDownload.title')" size="small">
        <template #header>
          <Download :size="20" />
        </template>
        <div class="flex gap-3">
          <Button
            type="big"
            :title="$t('settings.text.engineDownload.cpuHint')"
            @click="
              openWebsite(
                'https://www.modelscope.cn/models/lingchat-research-studio/SBV2-API/files'
              )
            "
            >{{ $t("settings.text.engineDownload.cpu") }}</Button
          >
          <Button
            type="big"
            @click="
              openWebsite(
                'https://www.modelscope.cn/models/lingchat-research-studio/Style-Bert-VITS2-CUDA/files'
              )
            "
            >{{ $t("settings.text.engineDownload.nvidia") }}</Button
          >
          <Button
            type="big"
            :title="$t('settings.text.engineDownload.amdHint')"
            @click="
              openWebsite(
                'https://www.modelscope.cn/models/lingchat-research-studio/SBV2-API/files'
              )
            "
            >{{ $t("settings.text.engineDownload.amd") }}</Button
          >
        </div>
      </MenuItem>

      <MenuItem :title="$t('settings.text.back.title')" size="small">
        <template #header>
          <ArrowBigLeft :size="20" />
        </template>
        <div class="flex gap-3">
          <Button type="big" @click="returnToMain">{{ $t("settings.text.back.button") }}</Button>
          <Button type="big" @click="refreshTTS">{{ $t("settings.text.back.refreshTts") }}</Button>
          <Button v-if="isFreeDialogMode" type="big" variant="danger" @click="handleClearHistory">{{
            $t("settings.text.back.clearHistory")
          }}</Button>
        </div>
      </MenuItem>

      <!-- ─── 语音缓存 ──────────────────────────────── -->
      <MenuItem :title="$t('settings.text.ttsCache.title')" size="small">
        <template #header>
          <HardDrive :size="20" />
        </template>
        <div class="w-full space-y-2">
          <div class="flex items-center justify-between text-base">
            <span class="text-gray-50">{{ $t("settings.text.ttsCache.current") }}</span>
            <span class="font-medium text-gray-50">{{ ttsCacheSize }}</span>
          </div>
          <div class="text-xs text-gray-50/70">
            {{ $t("settings.text.ttsCache.files", { count: ttsCacheFiles }) }}
          </div>
          <div
            v-if="lastCleanupInfo && lastCleanupInfo.deleted > 0"
            class="text-xs text-emerald-300/90"
          >
            {{ $t("settings.text.ttsCache.lastCleanup", { count: lastCleanupInfo.deleted }) }}
          </div>
          <div class="text-xs text-gray-50/70">
            {{
              $t("settings.text.ttsCache.orphan", { count: ttsOrphanFiles, size: ttsOrphanSize })
            }}
          </div>
          <div class="flex gap-3 pt-1">
            <Button type="big" @click="checkTtsCache">
              <RefreshCw :size="16" class="mr-1" />
              {{ $t("settings.text.ttsCache.check") }}
            </Button>
            <Button type="big" @click="handleClearTtsCache">
              <Trash2 :size="16" class="mr-1" />
              {{ $t("settings.text.ttsCache.clean") }}
            </Button>
          </div>
        </div>
      </MenuItem>

      <!-- ─── Codex 额度（仅对话模型选择 Codex 提供商时显示） ──────────────── -->
      <MenuItem v-if="showCodexQuota" :title="$t('settings.text.codexQuota.title')" size="small">
        <template #header>
          <Gauge :size="20" />
        </template>
        <div class="w-full space-y-2">
          <template v-if="codexLoggedIn && codexDisplayRows.length > 0">
            <div v-for="row in codexDisplayRows" :key="row.label" class="space-y-1">
              <div class="flex items-center justify-between text-base">
                <span class="text-gray-50">{{ row.label }}</span>
                <span class="font-medium text-gray-50">
                  {{
                    $t("settings.text.codexQuota.remaining", {
                      percent: Math.round(row.window.remaining_percent),
                    })
                  }}
                </span>
              </div>
              <div class="h-2 w-full overflow-hidden rounded-full bg-slate-700/50">
                <div
                  class="h-full rounded-full bg-cyan-400 transition-all duration-300"
                  :style="{ width: `${row.window.remaining_percent}%` }"
                ></div>
              </div>
              <div v-if="row.window.reset_at" class="text-xs text-gray-50/70">
                {{
                  $t("settings.text.codexQuota.resetAt", {
                    time: formatResetAt(row.window.reset_at),
                  })
                }}
              </div>
            </div>
          </template>
          <div v-else-if="!codexLoggedIn" class="text-xs text-gray-50/70">
            {{ $t("settings.text.codexQuota.notLoggedIn") }}<br />
            {{ $t("settings.text.codexQuota.loginHint") }}
          </div>
          <div v-if="codexQuotaError" class="text-xs text-red-300/90">
            {{ $t("settings.text.codexQuota.loadFailed", { error: codexQuotaError }) }}
          </div>
          <div v-if="codexFastMode" class="flex items-center gap-1 text-xs text-amber-300/90">
            <Zap :size="12" />
            {{ $t("settings.text.codexQuota.fastModeHint") }}
          </div>
          <div class="flex gap-3 pt-1">
            <Button type="big" :disabled="codexQuotaLoading" @click="refreshCodexQuota">
              <RefreshCw :size="16" class="mr-1" :class="{ 'animate-spin': codexQuotaLoading }" />
              {{ $t("settings.text.codexQuota.refresh") }}
            </Button>
          </div>
        </div>
      </MenuItem>

      <!-- ─── 版本更新 ──────────────────────────────── -->
      <MenuItem :title="$t('settings.text.update.title')" size="small">
        <template #header>
          <RefreshCw :size="20" :class="{ 'animate-spin': updateChecking }" />
        </template>
        <div class="w-full space-y-2">
          <!-- 程序版本 -->
          <div class="flex items-center justify-between text-base">
            <span class="text-gray-50">{{ $t("settings.text.update.appVersion") }}</span>
            <span class="text-gray-50">v{{ currentAppVersion }}</span>
          </div>
          <!-- 数据版本 -->
          <div class="flex items-center justify-between text-base">
            <span class="text-gray-50">{{ $t("settings.text.update.dataVersion") }}</span>
            <span class="text-gray-50">v{{ currentDataVersion }}</span>
          </div>
          <!-- 状态文字（内联显示，不用 modal） -->
          <div v-if="updateStatusText" :class="updateStatusColor" class="text-sm font-medium">
            {{ updateStatusText }}
          </div>
          <!-- 下载进度条 -->
          <div
            v-if="updatePhase === 'downloading'"
            class="h-2 w-full overflow-hidden rounded-full bg-slate-700/50"
          >
            <div
              class="h-full rounded-full bg-cyan-400 transition-all duration-300"
              :style="{ width: `${downloadProgress}%` }"
            ></div>
          </div>
          <div class="flex gap-3 pt-1">
            <Button
              type="big"
              @click="handleCheckUpdate"
              :disabled="updateChecking || updatePhase === 'downloading'"
            >
              {{
                updateChecking
                  ? $t("settings.text.update.checking")
                  : $t("settings.text.update.checkButton")
              }}
            </Button>
            <Button
              v-if="updateAvailable"
              type="big"
              variant="primary"
              :disabled="updatePhase === 'downloading'"
              @click="handleInstallUpdate"
            >
              {{
                updatePhase === "downloading"
                  ? $t("settings.text.update.downloading")
                  : $t("settings.text.update.updateTo", { version: updateLatestVersion })
              }}
            </Button>
            <Button
              v-if="resourceSyncAvailable && updatePhase !== 'downloading'"
              type="big"
              @click="handleCheckResourceSync"
            >
              {{ $t("settings.text.update.syncData") }}
            </Button>
          </div>
          <!-- 资源同步对话框 -->
          <ResourceSyncDialog
            :visible="showResourceSyncDialog"
            :phase="resourceSyncPhase"
            :sync-info="resourceSyncInfo"
            :error-message="resourceSyncError"
            @apply="handleApplyResourceSync"
            @close="handleResourceSyncClose"
          />
        </div>
      </MenuItem>

      <!-- ─── 局域网同步 ──────────────────────────────── -->
      <MenuItem :title="$t('settings.text.lanSync.title')" size="small">
        <template #header>
          <Wifi :size="20" />
        </template>
        <div class="w-full space-y-2">
          <p class="text-sm text-gray-50/70">
            {{ $t("settings.text.lanSync.desc") }}
          </p>
          <div class="flex gap-3 pt-1">
            <Button type="big" @click="openLanSync">
              {{ $t("settings.text.lanSync.open") }}
            </Button>
          </div>
          <!-- 局域网同步对话框 -->
          <LanSyncDialog
            :visible="lanSync.dialogVisible.value"
            :view="lanSyncView"
            :phase="lanSync.phase.value"
            :server-port="lanSync.serverPort.value"
            :peers="lanSync.peers.value"
            :sync-plan="lanSync.syncPlan.value"
            :progress="lanSync.progress.value"
            :last-result="lanSync.lastResult.value"
            :error-message="lanSync.errorMessage.value"
            @close="lanSync.closeDialog()"
            @rescan="lanSync.scanPeers()"
            @pull="
              (peer) => {
                lanSync.selectPeer(peer);
                lanSync.planPull();
              }
            "
            @push="
              (peer) => {
                lanSync.selectPeer(peer);
                lanSync.planPush();
              }
            "
            @confirm="handleLanSyncConfirm"
            @cancel="lanSync.reset()"
            @restart="lanSync.restart()"
          />
        </div>
      </MenuItem>
      <!-- ─── 相关文档 ──────────────────────────────── -->
      <MenuItem :title="$t('settings.text.docs.title')" size="small">
        <template #header>
          <BookOpen :size="20" />
        </template>
        <div class="w-full space-y-2">
          <p class="text-sm text-gray-50/70">
            {{ $t("settings.text.docs.desc") }}
          </p>
          <div class="flex gap-3 pt-1">
            <Button
              type="big"
              @click="
                openWebsite(
                  'https://slimeboyowo.github.io/LingBlog/blog/projects/ling-chat/develop/'
                )
              "
              >{{ $t("settings.text.docs.button") }}</Button
            >
          </div>
        </div>
      </MenuItem>
    </MenuPage>
  </div>
</template>

<script setup lang="ts">
  import {
    codexAuthStatus,
    codexGetQuota,
    type CodexUsage,
    type QuotaWindow,
  } from "@/api/services/codex";
  import type { ConfigItem } from "@/api/services/config";
  import { getEnvConfigByKey, saveEnvConfigSettings } from "@/api/services/config";
  import {
    clearImportedFontsCache,
    getImportedFonts,
    importFont,
    listSystemFonts,
    registerFontFace,
    type ImportedFontInfo,
  } from "@/api/services/font";
  import type { WebInitData } from "@/api/services/game-info";
  import { clearTtsCache, reactivateTTS } from "@/api/services/game-info";
  import LanSyncDialog from "@/components/LanSyncDialog.vue";
  import ResourceSyncDialog from "@/components/ResourceSyncDialog.vue";
  import { useLanSync } from "@/composables/useLanSync";
  import { useUpdater } from "@/composables/useUpdater";
  import { applyWebInitData } from "@/stores/modules/game/actions";
  import { useLlmProvidersStore } from "@/stores/modules/llm-providers";
  import type { DialogView } from "@/types/lanSync";
  import { getVersion } from "@tauri-apps/api/app";
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import {
    AlignJustify,
    ArrowBigLeft,
    BookOpen,
    Bug,
    ClipboardList,
    Combine,
    Download,
    Earth,
    Gauge,
    GlassWater,
    HardDrive,
    Import,
    RefreshCw,
    Star,
    Timer,
    Trash2,
    Type,
    Wifi,
    Zap,
  } from "lucide-vue-next";
  import { computed, onMounted, onUnmounted, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { useRouter } from "vue-router";
  import { useGameStore } from "../../../stores/modules/game";
  import { useSettingsStore } from "../../../stores/modules/settings";
  import { useDialogStore } from "../../../stores/modules/ui/dialog";
  import { useRoleArchiveStore } from "../../../stores/modules/ui/role-archive";
  import { useUIStore } from "../../../stores/modules/ui/ui";
  import { Button, Slider, Text, Toggle } from "../../base";
  import { MenuItem, MenuPage } from "../../ui";

  const router = useRouter();
  const { t } = useI18n();
  // 模板里不能用 import.meta（模板表达式按非 module 解析），这里提前取出
  const isDevMode = import.meta.env.DEV;
  const uiStore = useUIStore();
  const roleStore = useRoleArchiveStore();
  const settingsStore = useSettingsStore();
  const gameStore = useGameStore();
  const dialogStore = useDialogStore();
  const envSettings = ref<Record<string, ConfigItem>>({});
  const ttsCacheSize = ref("0 B");
  const ttsCacheFiles = ref(0);
  const ttsOrphanFiles = ref(0);
  const ttsOrphanSize = ref("0 B");
  const lastCleanupInfo = ref<{ deleted: number; timestamp: number } | null>(null);
  let ttsCacheRefreshTimer: ReturnType<typeof setInterval> | null = null;

  // ─── Codex 额度 ────────────────────────────────────────────────
  // 只有「对话模型」选择了 Codex 提供商时才显示额度卡片并轮询；
  // 切换为其他模型后卡片即时隐藏、轮询停止。
  const llmProvidersStore = useLlmProvidersStore();
  const showCodexQuota = computed(() => llmProvidersStore.chatProvider?.provider === "codex");
  // Fast Mode（service_tier=priority）按 1.5 倍速度消耗额度，开启时在卡片上提示
  const codexFastMode = computed(() => llmProvidersStore.chatProvider?.fast_mode === true);
  const codexLoggedIn = ref(false);
  const codexUsage = ref<CodexUsage | null>(null);
  const codexQuotaError = ref("");
  const codexQuotaLoading = ref(false);
  let codexQuotaTimer: ReturnType<typeof setInterval> | null = null;

  interface CodexQuotaRow {
    label: string;
    window: QuotaWindow;
  }

  // 把主桶与额外桶（如 GPT-5.3-Codex-Spark）按窗口展平成显示行：
  // 按 window_seconds 归类（18000=5 小时窗，604800=7 天周窗），
  // 不认识的窗口长度也照样显示（后端文案里标注秒数）。
  const codexDisplayRows = computed<CodexQuotaRow[]>(() => {
    const usage = codexUsage.value;
    if (!usage) return [];
    const rows: CodexQuotaRow[] = [];
    const pushWindow = (prefix: string, window: QuotaWindow | null) => {
      if (!window) return;
      const suffix =
        window.window_seconds === 18000
          ? t("settings.text.codexQuota.fiveHour")
          : window.window_seconds === 604800
            ? t("settings.text.codexQuota.weekly")
            : `${window.window_seconds}s`;
      rows.push({ label: prefix ? `${prefix} · ${suffix}` : suffix, window });
    };
    pushWindow("", usage.rate_limit.secondary);
    pushWindow("", usage.rate_limit.primary);
    for (const extra of usage.additional) {
      pushWindow(extra.name, extra.quota.secondary);
      pushWindow(extra.name, extra.quota.primary);
    }
    return rows;
  });

  function formatResetAt(resetAt: number): string {
    return new Date(resetAt * 1000).toLocaleString();
  }

  async function refreshCodexQuota() {
    if (codexQuotaLoading.value) return;
    codexQuotaLoading.value = true;
    codexQuotaError.value = "";
    try {
      const status = await codexAuthStatus();
      codexLoggedIn.value = status.logged_in;
      codexUsage.value = status.logged_in ? await codexGetQuota() : null;
    } catch (e: any) {
      codexUsage.value = null;
      codexQuotaError.value = String(e?.message ?? e);
    } finally {
      codexQuotaLoading.value = false;
    }
  }

  // 判断是否在自由对话模式（没有运行剧本）
  const isFreeDialogMode = computed(() => gameStore.runningScript === null);

  // ─── 更新检查 ────────────────────────────────────────────────

  const updater = useUpdater();
  const {
    phase: updatePhase,
    appVersion: updateAppVersion,
    errorMessage: updateErrorMessage,
    downloadProgress,
    // 资源同步
    resourceSyncInfo,
    resourceSyncPhase,
    resourceSyncError,
    checkResourceSync,
    applyResourceSync,
    getDataVersion,
    resetResourceSync,
  } = updater;

  const currentAppVersion = ref("0.1.0");
  const currentDataVersion = ref(0);
  const updateLatestVersion = ref("");
  const updateChecking = ref(false);
  const showResourceSyncDialog = ref(false);
  const resourceSyncAvailable = ref(false);

  const updateAvailable = computed(
    () => updateLatestVersion.value !== "" && updatePhase.value === "app-update-available"
  );

  const updateStatusText = computed(() => {
    if (updatePhase.value === "checking") return t("settings.text.update.statusChecking");
    if (updatePhase.value === "downloading")
      return t("settings.text.update.statusDownloading", { progress: downloadProgress.value });
    if (updatePhase.value === "complete") return t("settings.text.update.statusComplete");
    if (updatePhase.value === "error")
      return updateErrorMessage.value || t("settings.text.update.statusError");
    if (updateAvailable.value) return t("settings.text.update.statusAvailable");
    return "";
  });

  const updateStatusColor = computed(() => {
    if (updatePhase.value === "error") return "text-red-400";
    if (updateAvailable.value) return "text-amber-400";
    if (updatePhase.value === "complete") return "text-green-400";
    return "text-green-400";
  });

  async function loadAppVersion() {
    try {
      currentAppVersion.value = await getVersion();
    } catch {
      // 使用默认值
    }
  }

  async function loadDataVersion() {
    currentDataVersion.value = await getDataVersion();
  }

  /** 进入页面时自动检查一次（静默，失败不弹窗） */
  async function autoCheckUpdate() {
    try {
      const hasUpdate = await updater.checkForUpdates();
      if (hasUpdate) {
        updateLatestVersion.value = updateAppVersion.value;
      }
      // 自动检查失败：重置错误状态，不显示任何提示
    } catch {
      updater.reset();
    }
  }

  async function handleCheckUpdate() {
    updateChecking.value = true;
    updateLatestVersion.value = "";
    try {
      const hasUpdate = await updater.checkForUpdates();
      if (hasUpdate) {
        updateLatestVersion.value = updateAppVersion.value;
      }
      // 失败或错误状态通过 updatePhase / updateStatusText 内联展示
    } finally {
      updateChecking.value = false;
    }
  }

  /** 直接安装更新（下载进度+状态全部内联） */
  async function handleInstallUpdate() {
    try {
      await updater.installAppUpdate();
      // 成功：phase 变为 'complete'，自动重启
    } catch {
      // 错误通过 phase 内联展示
    }
  }

  async function handleCheckResourceSync() {
    const hasUpdate = await checkResourceSync();
    if (hasUpdate) {
      showResourceSyncDialog.value = true;
    }
    // 刷新数据版本号
    await loadDataVersion();
  }

  async function handleApplyResourceSync(selectedFiles: string[]) {
    await applyResourceSync(selectedFiles);
    // 刷新数据版本号
    await loadDataVersion();
  }

  function handleResourceSyncClose() {
    showResourceSyncDialog.value = false;
    resetResourceSync();
  }

  // ─── 局域网同步 ────────────────────────────────────────────────

  const lanSync = useLanSync();
  const lanSyncView = ref<DialogView>("device-list");

  // 监听阶段变化，自动切换视图
  watch(
    () => lanSync.phase.value,
    (newPhase) => {
      switch (newPhase) {
        case "idle":
        case "scanning":
          lanSyncView.value = "device-list";
          break;
        case "planning":
          lanSyncView.value = "sync-plan";
          break;
        case "executing":
          lanSyncView.value = "progress";
          break;
        case "complete":
        case "error":
          lanSyncView.value = "result";
          break;
      }
    }
  );

  async function openLanSync() {
    lanSync.init();
    await lanSync.openDialog();
    lanSyncView.value = "device-list";
  }

  async function handleLanSyncConfirm() {
    const plan = lanSync.syncPlan.value;
    if (!plan) return;
    lanSyncView.value = "progress";
    if (plan.direction === "pull") {
      await lanSync.executePull();
    } else {
      await lanSync.executePush();
    }
  }

  // 加载版本号、预检更新和数据同步
  loadAppVersion();
  loadDataVersion();
  autoCheckUpdate();
  checkResourceSyncAvailability();

  async function checkResourceSyncAvailability() {
    try {
      const info = await checkResourceSync();
      resourceSyncAvailable.value = info;
    } catch {
      resourceSyncAvailable.value = false;
    }
  }

  const returnToMain = () => {
    uiStore.toggleSettings(false);
    router.push("/");
  };

  const handleClearHistory = async () => {
    // 提示用户保存
    const confirmed = await dialogStore.confirm(t("settings.text.clearHistory.confirm"));
    if (!confirmed) return;

    try {
      // 调用后端重置对话（复用 init_game_status 逻辑）
      const data = await invoke<WebInitData>("clear_conversation");
      applyWebInitData(gameStore.$state, data);

      // 重置前端输入状态
      gameStore.currentStatus = "input";
      gameStore.currentLine = "";

      // 重置 UI 状态
      uiStore.currentBackgroundMusic = "None";
      uiStore.currentAvatarAudio = "None";
      uiStore.bgMusicPaused = false;
      uiStore.bgMusicStoped = true;

      // 清除运行中的剧本状态
      gameStore.exitStoryMode();

      uiStore.showNotification({
        type: "success",
        title: t("settings.text.clearHistory.successTitle"),
        message: t("settings.text.clearHistory.successMessage"),
        duration: 3000,
        skipTipsCheck: true,
      });
    } catch (error: any) {
      uiStore.showNotification({
        type: "error",
        title: t("settings.text.clearHistory.errorTitle"),
        message: error.message || t("settings.text.clearHistory.errorMessage"),
        duration: 3000,
        skipTipsCheck: true,
      });
    }
  };

  onMounted(async () => {
    loadConfig();
    checkTtsCache();
    loadLastTtsCleanup();
    // 加载本机已装字体族列表（Rust 侧枚举，单次缓存）
    void loadSystemFonts();
    // 加载已导入字体列表
    void loadImportedFonts();
    // 每 30 秒自动刷新一次 TTS 缓存信息，频率适中不浪费资源
    ttsCacheRefreshTimer = setInterval(() => {
      checkTtsCache();
    }, 30000);
    // Codex 额度卡片依赖对话模型的提供商类型，先确保模型列表已加载
    if (!llmProvidersStore.loaded) {
      await llmProvidersStore.load();
    }
    // Codex 额度：对话模型是 Codex 时才查，之后每 60 秒轮询
    if (showCodexQuota.value) {
      refreshCodexQuota();
      startCodexQuotaPolling();
    }
  });

  // 对话模型切到/切出 Codex 时，即时显隐卡片并启停轮询
  watch(showCodexQuota, (show) => {
    if (show) {
      refreshCodexQuota();
      startCodexQuotaPolling();
    } else {
      stopCodexQuotaPolling();
    }
  });

  function startCodexQuotaPolling() {
    stopCodexQuotaPolling();
    codexQuotaTimer = setInterval(() => {
      refreshCodexQuota();
    }, 60000);
  }

  function stopCodexQuotaPolling() {
    if (codexQuotaTimer) {
      clearInterval(codexQuotaTimer);
      codexQuotaTimer = null;
    }
  }

  onUnmounted(() => {
    if (ttsCacheRefreshTimer) {
      clearInterval(ttsCacheRefreshTimer);
      ttsCacheRefreshTimer = null;
    }
    stopCodexQuotaPolling();
  });

  function loadLastTtsCleanup() {
    try {
      const raw = localStorage.getItem("lingchat:last_tts_cleanup");
      if (raw) {
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed.deleted === "number") {
          lastCleanupInfo.value = {
            deleted: parsed.deleted,
            timestamp: parsed.timestamp ?? 0,
          };
        }
      }
    } catch (error: any) {
      console.error("读取 TTS 清理记录失败:", error);
    }
  }

  const loadConfig = async () => {
    const configKeys = ["features.use_persistent_memory"];
    for (const key of configKeys) {
      envSettings.value[key] = await getEnvConfigByKey(key);
    }
  };

  // 使用 settings store 的文字速度
  const textSpeed = computed({
    get: () => settingsStore.textSpeed,
    set: (val: number) => settingsStore.update("text.speed", val),
  });

  // 使用 settings store 的自动推进延迟（毫秒）
  const autoAdvanceDelay = computed({
    get: () => settingsStore.autoAdvanceDelay,
    set: (val: number) => settingsStore.update("text.autoAdvanceDelay", val),
  });

  // 台词合并阈值（字符数，仅计聊天文本；0=关闭合并）
  const mergeLineThreshold = computed({
    get: () => settingsStore.text.mergeLineThreshold,
    set: (val: number) => settingsStore.update("text.mergeLineThreshold", val),
  });

  // 台词合并续打延迟（ms）
  const mergeLineDelay = computed({
    get: () => settingsStore.text.mergeLineDelay,
    set: (val: number) => settingsStore.update("text.mergeLineDelay", val),
  });

  // 台词合并时动作文本的处理方式：append=接在后面显示（| 分隔）/ replace=独立显示本次动作
  const mergeMotionMode = computed<"append" | "replace">({
    get: () => settingsStore.text.mergeMotionMode ?? "append",
    set: (val) => settingsStore.update("text.mergeMotionMode", val),
  });
  const setMergeMotionMode = (val: "append" | "replace") => {
    settingsStore.update("text.mergeMotionMode", val);
  };

  // ─── 界面字体选择 ───────────────────────────────────────────
  const systemFonts = ref<string[]>([]);
  const importedFonts = ref<ImportedFontInfo[]>([]);
  const fontsLoading = ref(false);
  const fontFamily = computed({
    get: () => settingsStore.text.fontFamily ?? "",
    set: (val: string) => settingsStore.update("text.fontFamily", val),
  });
  const SOFTWARE_DEFAULT_STACK =
    "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Microsoft YaHei', sans-serif";
  const demoFontFamily = computed(() =>
    fontFamily.value ? `'${fontFamily.value}'` : SOFTWARE_DEFAULT_STACK
  );
  function onFontChange() {
    // fontFamily 是 computed setter，已写入 store；App.vue 的 watcher 会应用
  }
  async function loadSystemFonts() {
    fontsLoading.value = true;
    try {
      const list = await listSystemFonts();
      const zh = list.filter((n) => /[一-鿿぀-ヿ]/.test(n));
      const rest = list.filter((n) => !zh.includes(n));
      systemFonts.value = [...zh, ...rest];
    } finally {
      fontsLoading.value = false;
    }
  }

  // ─── 导入字体 ───────────────────────────────────────────────
  async function loadImportedFonts() {
    try {
      const list = await getImportedFonts();
      importedFonts.value = list;
    } catch (e) {
      console.error("加载导入字体列表失败:", e);
      importedFonts.value = [];
    }
  }

  async function handleImportFont() {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: "字体文件", extensions: ["ttf", "otf", "woff", "woff2"] }],
    });
    if (!selected) return;

    const filePath = typeof selected === "string" ? selected : (selected as any).path;
    if (!filePath) return;

    try {
      const result = await importFont(filePath);
      registerFontFace(result.font_family, result.file_path);
      clearImportedFontsCache();
      await loadImportedFonts();
      // 发生自动修正时弹顶部 amber notice 提示用户
      if (result.was_corrected) {
        const originalExt = result.original_name.split(".").pop() || "";
        roleStore.showCorrected({
          title: t("ui.notice.autoCorrected.title"),
          message: t("ui.notice.autoCorrected.font", {
            original: result.original_name,
            originalExt,
            detected: result.detected_kind,
            corrected: result.actual_name,
          }),
        });
      }
      uiStore.showNotification({
        type: "success",
        title: "字体导入成功",
        message: `字体 "${result.font_family}" 已导入`,
        duration: 3000,
        skipTipsCheck: true,
      });
    } catch (error: any) {
      const rawMsg = typeof error === "string" ? error : error?.message || String(error);
      const translated =
        rawMsg === "FONT_INVALID_FORMAT" ? t("ui.fontImport.errors.FONT_INVALID_FORMAT") : rawMsg;
      uiStore.showNotification({
        type: "error",
        title: "字体导入失败",
        message: translated,
        duration: 3000,
        skipTipsCheck: true,
      });
    }
  }

  // 文字样本速度（响应式）
  const textSpeedSample = ref<number>(settingsStore.textSpeed);

  const textSpeedChange = (data: number) => {
    settingsStore.update("text.speed", data);
    textSpeedSample.value = data;
  };

  const voiceSound = (data: boolean) => {
    settingsStore.update("audio.chatEffectSound", data);
  };

  const toggleInlineMotionText = (data: boolean) => {
    settingsStore.update("text.inlineMotionText", data);
  };

  const toggleSedentaryReminder = (data: boolean) => {
    settingsStore.update("text.sedentaryReminder", data);
  };

  const toggleVueDevTools = (data: boolean) => {
    settingsStore.update("text.vueDevToolsEnabled", data);
  };

  const handleMemorySettingChange = (checked: boolean, setting: ConfigItem) => {
    const newValue = checked ? "true" : "false";
    setting.value = newValue;

    const formData: Record<string, string> = {};
    Object.entries(envSettings.value).forEach(([key, config]) => {
      formData[key] = config.value;
    });
    saveEnvConfigSettings(formData);
  };

  const openWebsite = (url: string) => {
    openUrl(url);
  };

  const refreshTTS = async () => {
    try {
      await reactivateTTS();
      await dialogStore.alert(t("settings.text.refreshTts.success"));
    } catch (error) {
      await dialogStore.alert(t("settings.text.refreshTts.error"));
    }
  };

  const handleClearTtsCache = async () => {
    try {
      const result = await clearTtsCache();
      await checkTtsCache();
      uiStore.showNotification({
        type: result.success ? "success" : "warning",
        title: result.success
          ? t("settings.text.ttsCache.cleanSuccess")
          : t("settings.text.ttsCache.cleanDone"),
        message: result.message,
        duration: 3000,
        skipTipsCheck: true,
      });
    } catch (error: any) {
      uiStore.showNotification({
        type: "error",
        title: t("settings.text.ttsCache.cleanErrorTitle"),
        message: error.message || t("settings.text.ttsCache.cleanErrorMessage"),
        duration: 3000,
        skipTipsCheck: true,
      });
    }
  };

  async function checkTtsCache() {
    try {
      const result = await invoke<{
        size: number;
        files: number;
        orphan_size: number;
        orphan_files: number;
      }>("get_tts_cache_info");
      ttsCacheFiles.value = result.files;
      ttsCacheSize.value = formatBytes(result.size);
      ttsOrphanFiles.value = result.orphan_files;
      ttsOrphanSize.value = formatBytes(result.orphan_size);
    } catch (error: any) {
      console.error("获取TTS缓存信息失败:", error);
      ttsCacheSize.value = t("settings.text.ttsCache.unknown");
      ttsCacheFiles.value = 0;
      ttsOrphanFiles.value = 0;
      ttsOrphanSize.value = t("settings.text.ttsCache.unknown");
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }
</script>

<style scoped>
  .settings-text-container {
    position: relative;
    width: 100%;
    height: 100%;
  }

  .font-demo {
    flex: 1 1 0;
    min-width: 0;
    line-height: 1.5rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    background: rgba(30, 41, 59, 0.6);
    color: #f8fafc;
    border: 1px solid rgba(148, 163, 184, 0.4);
    border-radius: 0.5rem;
    padding: 0.35rem 0.6rem;
    font-size: 0.875rem;
    outline: none;
  }
  select option {
    background: #1e293b;
    color: #f8fafc;
  }
</style>
