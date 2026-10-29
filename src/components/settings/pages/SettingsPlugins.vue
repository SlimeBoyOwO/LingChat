<template>
  <MenuPage>
    <MenuItem :title="$t('settings.plugins.title')">
      <template #header>
        <Icon icon="package" :size="20" />
      </template>

      <!-- 改过 manifest 或往 data/plugins 里换过文件：重新扫描一次就生效，不用重启应用 -->
      <div class="mb-3 flex items-start justify-between gap-3">
        <p class="text-[11px] leading-relaxed text-white/40">
          {{ $t("settings.plugins.reloadHint") }}
        </p>
        <button
          type="button"
          class="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs text-white/70 transition-colors hover:bg-white/10 hover:text-white disabled:opacity-50"
          :disabled="reloading"
          @click="handleReload"
        >
          <RotateCw :size="13" :class="reloading ? 'animate-spin' : ''" />
          {{ $t("settings.plugins.reload") }}
        </button>
      </div>

      <!-- 错误提示 -->
      <div
        v-if="error"
        class="mb-4 rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-2.5 text-sm text-red-200"
      >
        {{ error }}
      </div>

      <div class="space-y-3">
        <div
          v-for="plugin in plugins"
          :key="plugin.id"
          class="rounded-xl border border-white/10 bg-white/5 p-4 transition-colors hover:border-white/20"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <h3 class="truncate text-base font-bold text-white">{{ plugin.name }}</h3>
                <span
                  class="shrink-0 rounded-full border border-white/10 px-2 py-0.5 text-[10px] text-white/60"
                >
                  v{{ plugin.version }}
                </span>
                <span
                  class="shrink-0 rounded-full border px-2 py-0.5 text-[10px]"
                  :class="
                    plugin.enabled
                      ? 'border-emerald-400/30 bg-emerald-400/10 text-emerald-300'
                      : 'border-white/10 bg-white/5 text-white/45'
                  "
                >
                  {{
                    plugin.enabled
                      ? $t("settings.plugins.status.enabled")
                      : $t("settings.plugins.status.disabled")
                  }}
                </span>
              </div>
              <p class="mt-0.5 line-clamp-2 text-xs leading-relaxed text-white/60">
                {{ plugin.description }}
              </p>
            </div>

            <div class="flex shrink-0 items-center gap-2">
              <Toggle
                class="shrink-0"
                :checked="plugin.enabled"
                :disabled="!!plugin.error"
                @change="(v: boolean) => toggle(plugin, v)"
              />
              <button
                type="button"
                class="flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs text-white/70 transition-colors hover:bg-white/10 hover:text-white"
                @click="editingPluginId = plugin.id"
              >
                <SlidersHorizontal :size="13" />
                {{ $t("settings.plugins.configure") }}
              </button>
            </div>
          </div>

          <!-- 声明的 WebSocket 连接（含实时状态） -->
          <div v-if="plugin.ws.length" class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1">
            <span
              v-for="conn in plugin.ws"
              :key="conn.id"
              class="flex items-center gap-1.5 text-[11px] text-white/60"
              :title="wsModeLabel(conn.mode)"
            >
              <span class="h-1.5 w-1.5 shrink-0 rounded-full" :class="wsStateDot(conn.state)" />
              <span class="font-mono">{{ conn.id }}</span>
              <span class="text-white/40">{{ wsStateLabel(conn.state) }}</span>
            </span>
          </div>

          <!-- 错误信息 -->
          <p v-if="plugin.error" class="mt-2 text-xs text-red-300">{{ plugin.error }}</p>

          <!-- 启动阶段未能运行的原因（前置未就绪 / 启动入口失败，插件已被自动禁用） -->
          <p v-if="plugin.startup_error" class="mt-2 text-xs whitespace-pre-line text-amber-300">
            {{ formatPluginError(plugin.startup_error) }}
          </p>
        </div>

        <p v-if="!plugins.length" class="py-8 text-center text-sm text-white/50">
          {{ $t("settings.plugins.empty") }}
        </p>
      </div>
    </MenuItem>

    <PluginSettingsModal
      :visible="!!editingPlugin"
      :plugin="editingPlugin"
      @close="editingPluginId = null"
      @saved="handleModalSaved"
      @deleted="handleModalSaved"
    />

    <PluginArchiveProgress />

    <MenuItem :title="$t('settings.plugins.import.title')" size="small">
      <template #header>
        <PackageOpen :size="20" />
      </template>
      <div class="space-y-2">
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-medium text-white/60">{{
            $t("settings.plugins.import.conflictPolicy")
          }}</label>
          <select
            v-model="conflictPolicy"
            class="rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white transition-all duration-200 outline-none"
          >
            <option value="overwrite">{{ $t("settings.plugins.import.policyOverwrite") }}</option>
            <option value="abort">{{ $t("settings.plugins.import.policyAbort") }}</option>
          </select>
        </div>
        <Button type="big" @click="handleImport">{{ $t("settings.plugins.import.button") }}</Button>
        <p class="text-[11px] leading-relaxed text-white/40">
          {{ $t("settings.plugins.import.hint") }}
        </p>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed, onDeactivated, onMounted, onUnmounted, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { PackageOpen, RotateCw, SlidersHorizontal } from "@lucide/vue";
import { MenuPage, MenuItem } from "../../ui";
import { Button } from "../../base";
import Icon from "@/components/base/widget/Icon.vue";
import { Toggle } from "@/components/base";
import { i18n } from "@/locales";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import PluginArchiveProgress from "@/components/ui/PluginArchiveProgress.vue";
import { usePluginImport } from "@/composables/usePluginImport";
import PluginSettingsModal from "./plugins/PluginSettingsModal.vue";
import { formatPluginError, wsModeLabel, wsStateDot, wsStateLabel } from "./plugins/display";
import {
  listPlugins,
  reloadPlugins,
  setPluginEnabled,
  type PluginConflictPolicy,
  type PluginInfo,
  type WsState,
} from "@/api/services/plugins";

const plugins = ref<PluginInfo[]>([]);
const error = ref("");
const reloading = ref(false);
const editingPluginId = ref<string | null>(null);
const dialogStore = useDialogStore();

// 弹窗按 id 取当前列表里的对象：列表刷新后弹窗自动指向新对象，字典/连接状态也跟着更新
const editingPlugin = computed(
  () => plugins.value.find((p) => p.id === editingPluginId.value) ?? null,
);

/** 用弹窗而不是内联提示的错误码：都是「用户主动启用被拒」，需要解释该先做什么。 */
const DIALOG_ERROR_CODES = new Set([
  "PLUGIN_MISSING_DEPENDENCY",
  "PLUGIN_INACTIVE_DEPENDENCY",
  "PLUGIN_DEPENDENCY_CYCLE",
]);

/** 后端连接生命周期事件 → 前端连接状态。 */
const WS_EVENT_STATE: Record<string, WsState> = {
  connect: "connected",
  disconnect: "stopped",
  error: "error",
};

const load = async () => {
  try {
    plugins.value = await listPlugins();
  } catch (e) {
    error.value = String(e);
  }
};

/** 重新扫描插件目录：改过 manifest、或往 data/plugins 里换过文件时用，不用重启应用。 */
const handleReload = async () => {
  if (reloading.value) return;
  reloading.value = true;
  error.value = "";
  try {
    await reloadPlugins();
    await load();
  } catch (e) {
    error.value = String(e);
  } finally {
    reloading.value = false;
  }
};

const toggle = async (plugin: PluginInfo, enabled: boolean) => {
  if (plugin.error) return;
  // 禁用会移除插件角色并级联删除其存档/记忆（重启用不恢复），破坏性操作前确认。
  if (!enabled && plugin.resources.includes("characters")) {
    const ok = await dialogStore.confirm(
      i18n.global.t("settings.plugins.disableCharactersConfirm", { name: plugin.name }),
    );
    if (!ok) return;
  }
  try {
    await setPluginEnabled(plugin.id, enabled);
    // 重新拉一次列表：开关后的连接状态与启动错误都在 plugin_list 里
    await load();
  } catch (e) {
    const raw = String(e);
    // 前置插件没装 / 没启用 / 成环：用户主动启用被拒，弹窗说明该先做什么。
    // 其余错误仍走页面顶部的内联提示。
    if (DIALOG_ERROR_CODES.has(raw.split("|")[0])) {
      await dialogStore.alert(
        formatPluginError(raw),
        i18n.global.t("settings.plugins.enableFailedTitle"),
      );
    } else {
      error.value = raw;
    }
  }
};

/** 弹窗里保存配置或删除插件后：关掉弹窗并刷新列表。 */
const handleModalSaved = async () => {
  editingPluginId.value = null;
  await load();
};

// ===== 压缩包导入 =====

const { pickAndImport } = usePluginImport();
const conflictPolicy = ref<PluginConflictPolicy>("overwrite");

const handleImport = async () => {
  await pickAndImport(conflictPolicy.value);
  // 导入对话框关闭后（成功、失败或取消）都重新拉一次列表。
  await load();
};

let unlistenAutoDisabled: UnlistenFn | null = null;
let unlistenWsState: UnlistenFn | null = null;

onMounted(async () => {
  load();
  // 后端自动禁用插件（启动入口失败 / 前置未就绪 / 循环依赖）时同步 UI：
  // 只改状态，让开关滑块自己动画关闭并显示原因；**不弹窗**——弹窗留给
  // 用户主动启用被拒的场景，否则「刚点开又被推回去」还会再弹一次。
  unlistenAutoDisabled = await listen<{ id: string; reason: string }>(
    "plugin:auto-disabled",
    (event) => {
      const target = plugins.value.find((p) => p.id === event.payload.id);
      if (!target) return;
      target.enabled = false;
      target.startup_error = event.payload.reason;
    },
  );
  // 连接建立/断开/出错时就地更新圆点：外部程序断开后不必重进插件页
  unlistenWsState = await listen<{ pluginId: string; connId: string; event: string }>(
    "plugin:ws-state",
    (event) => {
      const { pluginId, connId, event: name } = event.payload ?? {};
      const state = WS_EVENT_STATE[name];
      if (!state) return;
      const conn = plugins.value.find((p) => p.id === pluginId)?.ws.find((c) => c.id === connId);
      if (conn) conn.state = state;
    },
  );
});

// 设置页被 KeepAlive 缓存：切到别的 tab 时关掉弹窗，切回来不会「复活」一个旧弹窗
onDeactivated(() => {
  editingPluginId.value = null;
});

onUnmounted(() => {
  unlistenAutoDisabled?.();
  unlistenAutoDisabled = null;
  unlistenWsState?.();
  unlistenWsState = null;
});
</script>
