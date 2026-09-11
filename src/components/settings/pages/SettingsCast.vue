<template>
  <div
    class="flex h-full min-h-0 w-full flex-wrap items-start gap-5 overflow-y-auto px-3 py-6
      text-white text-shadow-2xs"
  >
    <!-- 总开关（决定应用启动时是否自动开启投屏） -->
    <MenuItem :title="$t('settings.cast.enable')" size="small">
      <template #header>
        <Cast :size="20" class="text-cyan-400" />
      </template>
      <Toggle :checked="enabled" @change="onToggleEnabled">
        {{ $t("settings.cast.enableDesc") }}
      </Toggle>
    </MenuItem>

    <!-- 串流端口 -->
    <MenuItem :title="$t('settings.cast.port')" size="small">
      <template #header>
        <PlugZap :size="20" class="text-orange-400" />
      </template>
      <div class="flex w-full flex-col gap-2">
        <div class="flex w-full items-center gap-2">
          <input
            v-model.number="port"
            type="number"
            min="1"
            max="65535"
            class="cast-num-input"
            style="color-scheme: dark"
            @change="onPortChange"
          />
        </div>
        <p class="text-xs text-white/50">{{ $t("settings.cast.portDesc") }}</p>
      </div>
    </MenuItem>

    <!-- 性能参数：帧率 + 画质 -->
    <MenuItem :title="$t('settings.cast.performance')" size="small">
      <template #header>
        <Gauge :size="20" class="text-pink-400" />
      </template>
      <div class="flex w-full flex-col gap-3">
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70"
            >{{ $t("settings.cast.fps") }}（{{ fps }}）</span
          >
          <Slider v-model="fps" :min="1" :max="30" @change="onFpsChange">1/30</Slider>
        </div>
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70"
            >{{ $t("settings.cast.quality") }}（{{ quality }}）</span
          >
          <Slider v-model="quality" :min="1" :max="100" @change="onQualityChange">1/100</Slider>
        </div>
        <p class="text-xs text-white/50">{{ $t("settings.cast.perfHint") }}</p>
      </div>
    </MenuItem>

    <!-- 输出分辨率（0 = 跟随投屏窗口当前尺寸） -->
    <MenuItem :title="$t('settings.cast.resolution')" size="small">
      <template #header>
        <PictureInPicture2 :size="20" class="text-purple-400" />
      </template>
      <div class="flex w-full flex-col gap-2">
        <div class="flex w-full items-center gap-2">
          <input
            v-model.number="castWidth"
            type="number"
            min="0"
            max="1920"
            step="1"
            class="cast-num-input"
            style="color-scheme: dark"
            @change="onResolutionChange"
          />
          <span class="text-white/50">×</span>
          <input
            v-model.number="castHeight"
            type="number"
            min="0"
            max="1920"
            step="1"
            class="cast-num-input"
            style="color-scheme: dark"
            @change="onResolutionChange"
          />
        </div>
        <p class="text-xs text-white/50">{{ $t("settings.cast.resolutionDesc") }}</p>
      </div>
    </MenuItem>

    <!-- vivid 色彩增强：串流时饱和度/对比度预设（复刻 cast_sender.py 的 --vivid） -->
    <MenuItem :title="$t('settings.cast.vivid')" size="small">
      <template #header>
        <Palette :size="20" class="text-amber-400" />
      </template>
      <Toggle :checked="vivid" @change="onVividChange">
        {{ $t("settings.cast.vividDesc") }}
      </Toggle>
    </MenuItem>

    <!-- 隐藏对话框：投屏只展示背景 + 角色舞台 -->
    <MenuItem :title="$t('settings.cast.dialogHide.title')" size="small">
      <template #header>
        <MessageSquareOff :size="20" class="text-rose-400" />
      </template>
      <Toggle :checked="dialogHidden" @change="onDialogHiddenChange">
        {{ $t("settings.cast.dialogHide.desc") }}
      </Toggle>
    </MenuItem>

    <!-- 角色调整：缩放 + 水平/垂直偏移（作用于投屏窗口角色舞台层） -->
    <MenuItem :title="$t('settings.cast.characterAdjust')" size="small">
      <template #header>
        <Scan :size="20" class="text-orange-400" />
      </template>
      <div class="flex w-full flex-col gap-3">
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70">
            {{ $t("settings.cast.charScale") }}（{{ charScale.toFixed(2) }}）
          </span>
          <Slider v-model="charScale" :min="0.5" :max="2" :step="0.05" @change="onCharTuneChange"
            >0.5/2</Slider
          >
        </div>
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70">
            {{ $t("settings.cast.charOffsetX") }}（{{ Math.round(charOffsetX) }}px）
          </span>
          <Slider v-model="charOffsetX" :min="-400" :max="400" :step="10" @change="onCharTuneChange"
            >-400/400</Slider
          >
        </div>
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70">
            {{ $t("settings.cast.charOffsetY") }}（{{ Math.round(charOffsetY) }}px）
          </span>
          <Slider v-model="charOffsetY" :min="-400" :max="400" :step="10" @change="onCharTuneChange"
            >-400/400</Slider
          >
        </div>
        <p class="text-xs text-white/50">{{ $t("settings.cast.characterAdjustDesc") }}</p>
      </div>
    </MenuItem>

    <!-- 对话框调整：左右的留白（宽度） + 整体元素高度 -->
    <MenuItem :title="$t('settings.cast.dialogAdjust')" size="small">
      <template #header>
        <AlignLeft :size="20" class="text-sky-400" />
      </template>
      <div class="flex w-full flex-col gap-3">
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70">
            {{ $t("settings.cast.dialogWidth") }}（{{ Math.round(dialogWidth) }}%）
          </span>
          <Slider v-model="dialogWidth" :min="30" :max="100" :step="5" @change="onDialogTuneChange"
            >30/100</Slider
          >
        </div>
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70">
            {{ $t("settings.cast.dialogHeight") }}（{{ Math.round(dialogHeight) }}vh）
          </span>
          <Slider v-model="dialogHeight" :min="10" :max="100" :step="5" @change="onDialogTuneChange"
            >10/100</Slider
          >
        </div>
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70">
            {{ $t("settings.cast.dialogFontSize") }}（{{ Math.round(dialogFontSize) }}px）
          </span>
          <Slider
            v-model="dialogFontSize"
            :min="12"
            :max="40"
            :step="1"
            @change="onDialogTuneChange"
            >12/40</Slider
          >
        </div>
        <div class="w-full">
          <span class="mb-1 block text-xs text-white/70">
            {{ $t("settings.cast.dialogBgOpacity") }}（{{ Math.round(dialogBgOpacity) }}%）
          </span>
          <Slider
            v-model="dialogBgOpacity"
            :min="0"
            :max="100"
            :step="5"
            @change="onDialogTuneChange"
            >0/100</Slider
          >
        </div>
        <p class="text-xs text-white/50">{{ $t("settings.cast.dialogAdjustDesc") }}</p>
      </div>
    </MenuItem>

    <!-- 控制：开关窗口 / 启停串流 -->
    <MenuItem :title="$t('settings.cast.control')" size="small">
      <template #header>
        <MonitorPlay :size="20" class="text-green-400" />
      </template>
      <div class="flex w-full flex-wrap gap-2">
        <Button type="big" :disabled="castWindowOpen" @click="openWindow">
          {{ $t("settings.cast.openWindow") }}
        </Button>
        <Button type="big" :disabled="!castWindowOpen" @click="closeWindow">
          {{ $t("settings.cast.closeWindow") }}
        </Button>
        <Button type="big" :disabled="running" @click="startCast">
          {{ $t("settings.cast.start") }}
        </Button>
        <Button type="big" :disabled="!running" @click="stopCast">
          {{ $t("settings.cast.stop") }}
        </Button>
      </div>
      <p class="mt-2 text-xs text-white/50">{{ $t("settings.cast.windowHint") }}</p>
      <p v-if="statusMsg" class="mt-2 text-sm" :class="statusMsgColor">{{ statusMsg }}</p>
    </MenuItem>

    <!-- 连接信息卡 -->
    <MenuItem :title="$t('settings.cast.connection')" size="small">
      <template #header>
        <Wifi :size="20" class="text-blue-400" />
      </template>
      <template v-if="running">
        <div class="flex w-full flex-col gap-2">
          <div class="rounded-xl border border-white/10 bg-black/20 p-3">
            <div class="mb-1 text-xs text-white/50">{{ $t("settings.cast.streamUrlLabel") }}</div>
            <div class="cast-url">{{ streamUrl }}</div>
            <div class="mt-2 mb-1 text-xs text-white/50">
              {{ $t("settings.cast.pageUrlLabel") }}
            </div>
            <div class="cast-url">{{ pageUrl }}</div>
          </div>
          <p class="text-xs text-white/50">{{ $t("settings.cast.smallScreenHint") }}</p>
        </div>
      </template>
      <p v-else class="text-sm text-white/60">{{ $t("settings.cast.notRunning") }}</p>
    </MenuItem>
  </div>
</template>

<script setup lang="ts">
  import { saveEnvConfigSettings } from "@/api/services/config";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    AlignLeft,
    Cast,
    Gauge,
    MessageSquareOff,
    MonitorPlay,
    Palette,
    PictureInPicture2,
    PlugZap,
    Scan,
    Wifi,
  } from "lucide-vue-next";
  import { computed, onActivated, onMounted, onUnmounted, ref } from "vue";
  import { useI18n } from "vue-i18n";
  import { Button, Slider, Toggle } from "../../base";
  import { MenuItem } from "../../ui";

  interface CastStatus {
    enabled: boolean;
    port: number;
    fps: number;
    quality: number;
    width: number;
    height: number;
    vivid: boolean;
    charScale: number;
    charOffsetX: number;
    charOffsetY: number;
    dialogWidth: number;
    dialogHeight: number;
    dialogFontSize: number;
    dialogBgOpacity: number;
    dialogHidden: boolean;
    running: boolean;
    castWindowOpen: boolean;
    lanUrls: string[];
    pageUrl: string;
    streamUrl: string;
  }

  const { t } = useI18n();

  const status = ref<CastStatus | null>(null);
  const enabled = computed(() => status.value?.enabled ?? false);
  const running = computed(() => status.value?.running ?? false);
  const castWindowOpen = computed(() => status.value?.castWindowOpen ?? false);

  const port = ref(1470);
  const fps = ref(15);
  const quality = ref(80);
  const castWidth = ref(0);
  const castHeight = ref(0);
  const vivid = ref(false);
  const charScale = ref(1);
  const charOffsetX = ref(0);
  const charOffsetY = ref(0);
  const dialogWidth = ref(70);
  const dialogHeight = ref(40);
  const dialogFontSize = ref(20);
  const dialogBgOpacity = ref(70);
  const dialogHidden = ref(false);
  const streamUrl = ref("");
  const pageUrl = ref("");

  const statusMsg = ref("");
  const statusMsgColor = ref("text-white/60");
  let msgTimer: ReturnType<typeof setTimeout> | null = null;

  function setMsg(msg: string, color = "text-white/60") {
    statusMsg.value = msg;
    statusMsgColor.value = color;
    if (msgTimer) clearTimeout(msgTimer);
    msgTimer = setTimeout(() => {
      statusMsg.value = "";
    }, 6000);
  }

  async function refresh() {
    try {
      const s = await invoke<CastStatus>("cast_get_status");
      status.value = s;
      port.value = s.port;
      fps.value = s.fps;
      quality.value = s.quality;
      castWidth.value = s.width;
      castHeight.value = s.height;
      vivid.value = s.vivid;
      charScale.value = s.charScale ?? 1;
      charOffsetX.value = s.charOffsetX ?? 0;
      charOffsetY.value = s.charOffsetY ?? 0;
      dialogWidth.value = s.dialogWidth ?? 70;
      dialogHeight.value = s.dialogHeight ?? 40;
      dialogFontSize.value = s.dialogFontSize ?? 20;
      dialogBgOpacity.value = s.dialogBgOpacity ?? 70;
      dialogHidden.value = s.dialogHidden ?? false;
      streamUrl.value = s.streamUrl;
      pageUrl.value = s.pageUrl;
    } catch (e) {
      setMsg(t("settings.cast.loadFailed", { error: String(e) }), "text-red-400");
    }
  }

  /** 保存到后端 settings.json（Rust 侧读取的存储） */
  async function save(values: Record<string, string>): Promise<boolean> {
    try {
      await saveEnvConfigSettings(values);
      return true;
    } catch (e) {
      setMsg(t("settings.cast.saveFailed", { error: String(e) }), "text-red-400");
      return false;
    }
  }

  // ── 总开关 ────────────────────────────────────────────────
  async function onToggleEnabled(value: boolean) {
    if (!(await save({ "cast.enabled": String(value) }))) return;
    if (value) {
      // 打开时顺手现在就开一次，方便即时体验（不开的话要等下次启动才生效）
      await startCast();
    }
    await refresh();
  }

  // ── 参数变更 ──────────────────────────────────────────────
  async function onPortChange() {
    const p = Math.round(port.value);
    if (!(p >= 1 && p <= 65535)) {
      port.value = status.value?.port ?? 1470;
      return;
    }
    port.value = p;
    const ok = await save({ "cast.port": String(p) });
    if (ok) setMsg(t("settings.cast.portChangedRestartHint"));
  }

  async function onFpsChange() {
    await save({ "cast.fps": String(fps.value) });
  }

  async function onQualityChange() {
    await save({ "cast.quality": String(quality.value) });
  }

  // 输出分辨率：宽高都填 0 = 跟随投屏窗口当前尺寸
  async function onResolutionChange() {
    const w = Math.round(castWidth.value);
    const h = Math.round(castHeight.value);
    if (!(w >= 0 && w <= 1920 && h >= 0 && h <= 1920)) {
      castWidth.value = status.value?.width ?? 0;
      castHeight.value = status.value?.height ?? 0;
      return;
    }
    castWidth.value = w;
    castHeight.value = h;
    const ok = await save({ "cast.width": String(w), "cast.height": String(h) });
    if (ok) setMsg(t("settings.cast.resolutionChangedHint"));
  }

  // vivid 色彩增强：编码时生效（下一路串流连接即用新设置，无需重启服务）
  async function onVividChange(value: boolean) {
    if (!(await save({ "cast.vivid": String(value) }))) return;
    await refresh();
  }

  // 调参广播：保存后把整套参数发给已打开的投屏窗口即时生效（挂载时还有 cast_get_status 兜底）
  async function broadcastCastConfig() {
    try {
      await emit("cast:config", {
        charScale: charScale.value,
        charOffsetX: charOffsetX.value,
        charOffsetY: charOffsetY.value,
        dialogWidth: dialogWidth.value,
        dialogHeight: dialogHeight.value,
        dialogFontSize: dialogFontSize.value,
        dialogBgOpacity: dialogBgOpacity.value,
        dialogHidden: dialogHidden.value,
      });
    } catch (e) {
      console.warn("广播投屏调参失败:", e);
    }
  }

  // 角色调整（缩放 + 偏移）
  async function onCharTuneChange() {
    const ok = await save({
      "cast.char_scale": String(charScale.value),
      "cast.char_offset_x": String(charOffsetX.value),
      "cast.char_offset_y": String(charOffsetY.value),
    });
    if (!ok) return;
    setMsg(t("settings.cast.tuneChangedHint"));
    await broadcastCastConfig();
  }

  // 对话框调整（宽度 = 左右留白，高度 = 整体元素高度，字体大小，背景色透明度）
  async function onDialogTuneChange() {
    const ok = await save({
      "cast.dialog_width": String(dialogWidth.value),
      "cast.dialog_height": String(dialogHeight.value),
      "cast.dialog_font_size": String(dialogFontSize.value),
      "cast.dialog_bg_opacity": String(dialogBgOpacity.value),
    });
    if (!ok) return;
    setMsg(t("settings.cast.tuneChangedHint"));
    await broadcastCastConfig();
  }

  // 隐藏对话框：整层对话不显示（只留背景 + 角色舞台）
  async function onDialogHiddenChange(value: boolean) {
    if (!(await save({ "cast.dialog_hidden": String(value) }))) return;
    setMsg(t("settings.cast.tuneChangedHint"));
    await broadcastCastConfig();
  }

  // ── 窗口 / 服务控制 ───────────────────────────────────────
  async function openWindow() {
    try {
      await invoke("cast_open_window");
      await refresh();
      setMsg(t("settings.cast.windowOpen"), "text-green-400");
    } catch (e) {
      setMsg(t("settings.cast.actionFailed", { error: String(e) }), "text-red-400");
    }
  }

  async function closeWindow() {
    try {
      await invoke("cast_close_window");
      await refresh();
      setMsg(t("settings.cast.windowClosed"));
    } catch (e) {
      setMsg(t("settings.cast.actionFailed", { error: String(e) }), "text-red-400");
    }
  }

  async function startCast() {
    try {
      await invoke("cast_start");
      await refresh();
      setMsg(t("settings.cast.runningStatus"), "text-green-400");
    } catch (e) {
      setMsg(t("settings.cast.startFailed", { error: String(e) }), "text-red-400");
    }
  }

  async function stopCast() {
    try {
      await invoke("cast_stop");
      await refresh();
      setMsg(t("settings.cast.stopped"));
    } catch (e) {
      setMsg(t("settings.cast.stopFailed", { error: String(e) }), "text-red-400");
    }
  }

  // ── 生命周期 ─────────────────────────────────────────────
  let unlistenCastWindow: UnlistenFn | null = null;

  onMounted(async () => {
    await refresh();
    // 用户直接点投屏窗口的 X 关闭时，同步按钮状态
    unlistenCastWindow = await listen<boolean>("cast-window:state", async () => {
      await refresh();
    });
  });

  onActivated(async () => {
    // KeepAlive 缓存下重新切回本页时刷新一次状态
    await refresh();
  });

  onUnmounted(() => {
    if (msgTimer) clearTimeout(msgTimer);
    unlistenCastWindow?.();
  });
</script>

<style scoped>
  .cast-num-input {
    width: 120px;
    padding: 8px 10px;
    color: #fff;
    font-size: 14px;
    font-family: ui-monospace, Consolas, monospace;
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 8px;
    outline: none;
    transition:
      border-color 0.2s,
      box-shadow 0.2s;
  }
  .cast-num-input:focus {
    border-color: var(--accent-color);
    box-shadow: 0 0 0 3px rgba(0, 123, 255, 0.2);
  }

  .cast-url {
    font:
      13px/1.6 ui-monospace,
      Consolas,
      monospace;
    color: #71f59b;
    word-break: break-all;
    user-select: text;
  }
</style>
