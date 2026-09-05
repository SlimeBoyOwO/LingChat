<template>
  <div
    class="cast-shell fixed inset-0 z-0 overflow-hidden bg-black"
    :class="{ 'cast-hide-dialog': tune.dialogHidden }"
    :style="castShellStyle"
  >
    <!-- 无边框窗口的拖拽条：去掉原生标题栏后靠它移动窗口（透明，不干扰画面） -->
    <div class="cast-drag-region" data-tauri-drag-region></div>
    <!-- 背景层（背景图 + 粒子效果 + BGM） -->
    <div class="cast-layer z-0">
      <GameBackground />
    </div>
    <!-- 角色舞台层（Live2D 直接复用现有组件，天然支持）：
         缩放作为 cast-scale prop 注入 Live2D / 立绘布局（作用于模型本体，保持贴底定位）；
         垂直偏移作为 cast-offset-y prop 注入布局并在布局内夹紧（下移触底即止，
         人物下方不会被窗口 overflow:hidden 截断，且角色自身 offset 不被夹紧）；
         水平偏移经本层 .cast-role-layer 的 CSS translateX 整层平移（与最初版本一致的
         已验证方案，不依赖布局链重跑）。 -->
    <div class="cast-layer cast-role-layer z-1">
      <GameRolesStage :cast-scale="tune.charScale" :cast-offset-y="tune.charOffsetY" />
    </div>
    <!-- 对话层（打字机 / 情绪 / 动作文本）：
         复刻主界面 .main-box 的 flex column + justify-end，让对话框贴底居中显示 -->
    <div class="cast-layer cast-dialog-layer z-2">
      <GameDialog ref="gameDialogRef" />
    </div>

    <!-- 投屏交互按钮：左上「日程」、右上「菜单」（同主界面）。
         投屏窗口整体 pointer-events:none，这里单独放开。 -->
    <div class="cast-top-left">
      <SchedulePanel />
    </div>
    <div class="cast-top-right">
      <Button type="nav" icon="text" @click="openSettings" v-show="uiStore.showSettings !== true">
        <h3 class="m-0 text-lg font-bold">{{ $t("views.mainChat.menu") }}</h3>
      </Button>
    </div>
  </div>

  <!-- 设置面板（右上「菜单」打开）：独立于 cast-shell，避免 pointer-events:none 吞掉交互 -->
  <SettingsPanel />
</template>

<script setup lang="ts">
  import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { GameBackground, GameRolesStage, GameDialog } from "@/components/game/standard";
  import { Button } from "@/components/base";
  import SchedulePanel from "@/components/schedule/SchedulePanel.vue";
  import SettingsPanel from "@/components/settings/SettingsPanel.vue";
  import { useGameStore } from "@/stores/modules/game";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { listScenes, type SceneInfo } from "@/api/services/scene";

  /**
   * 主窗口镜像（App.vue 在台词变化时经 cast_emit_mirror 存储 + 广播 cast:mirror）。
   *
   * 投屏窗口与主窗口是各自独立的 webview / 事件队列。投屏若自己消费 ai:reply
   * 就会「按消息到达时间显示」，而主界面是等玩家逐句点击推进的——两条线脱节。
   * 镜像把主窗口「正在显示的完整状态」整包同步过来，投屏完全跟随主窗口。
   */
  interface CastMirror {
    line: string;
    title: string;
    subtitle: string;
    emotion: string;
    motionText: string;
    status: string;
    background: string | null;
    backgroundEffect: string | null;
    currentSceneId: string | null;
    presentRoleIds: number[];
    currentRoleId: number | null;
    /** 当前交互角色的 Live2D / 立绘表情（镜像携带，驱动表情切换） */
    roleEmotion: string;
    /** 当前交互角色的原始情绪标签（立绘目录映射用） */
    roleOriginalEmotion: string;
  }

  /** 设置页「角色调整 / 对话框调整」的投屏渲染参数（默认与主界面一致） */
  interface CastTune {
    charScale: number;
    charOffsetX: number;
    charOffsetY: number;
    dialogWidth: number;
    dialogHeight: number;
    dialogFontSize: number;
    dialogBgOpacity: number;
    /** 隐藏对话框：true 时对话层整体不显示，只留背景与角色舞台 */
    dialogHidden: boolean;
  }

  const gameStore = useGameStore();
  const uiStore = useUIStore();

  // 右上「菜单」：打开设置面板（同 MainChat.openSettings，投屏里直接调 settings）
  const openSettings = () => {
    uiStore.toggleSettings(true);
    uiStore.setSettingsTab("text");
  };

  const gameDialogRef = ref<InstanceType<typeof GameDialog> | null>(null);
  // 调参值：挂载时从 cast_get_status 读取，设置页改动经 cast:config 事件即时同步。
  // 缩放与偏移作为 prop 注入角色布局（偏移在布局内夹紧）；对话框经 CSS 变量。
  const tune = reactive<CastTune>({
    charScale: 1,
    charOffsetX: 0,
    charOffsetY: 0,
    dialogWidth: 70,
    dialogHeight: 40,
    dialogFontSize: 20,
    dialogBgOpacity: 70,
    dialogHidden: false,
  });
  const castShellStyle = computed<Record<string, string>>(() => {
    const bgAlpha = tune.dialogBgOpacity / 100;
    return {
      // 角色层水平偏移（CSS translateX 整层平移；垂直偏移走角色布局 prop）
      "--cast-char-offset-x": `${tune.charOffsetX}px`,
      "--cast-dialog-width": `${tune.dialogWidth}%`,
      "--cast-dialog-height": `${tune.dialogHeight}vh`,
      "--cast-dialog-font-size": `${tune.dialogFontSize}px`,
      // 背景渐变 alpha：底 0.7、顶 0.6（复刻主界面默认 dialogOpacity 0.7）
      "--cast-dialog-bg-alpha": String(bgAlpha),
      "--cast-dialog-bg-alpha-top": String(Math.max(0, bgAlpha - 0.1)),
    };
  });

  function applyTune(s: Partial<CastTune>) {
    if (typeof s.charScale === "number") tune.charScale = s.charScale;
    if (typeof s.charOffsetX === "number") tune.charOffsetX = s.charOffsetX;
    if (typeof s.charOffsetY === "number") tune.charOffsetY = s.charOffsetY;
    if (typeof s.dialogWidth === "number") tune.dialogWidth = s.dialogWidth;
    if (typeof s.dialogHeight === "number") tune.dialogHeight = s.dialogHeight;
    if (typeof s.dialogFontSize === "number") tune.dialogFontSize = s.dialogFontSize;
    if (typeof s.dialogBgOpacity === "number") tune.dialogBgOpacity = s.dialogBgOpacity;
    if (typeof s.dialogHidden === "boolean") tune.dialogHidden = s.dialogHidden;
  }

  // 场景缓存：避免每次对账都请求一次
  let scenesCache: SceneInfo[] = [];

  // ── 本地静音：投屏窗口只展示画面、不出声，避免与主窗口双重出声 ──
  // uiStore 的音量字段是 settings 的 getter（会持久化到用户全局设置），
  // 所以不能在投屏窗口里改它们，改在 HTMLMediaElement 原型层面拦截。
  function applyCastMute() {
    const muteEl = (el: HTMLMediaElement) => {
      el.muted = true;
      try {
        el.volume = 0;
      } catch {
        /* ignore */
      }
    };

    // 音量 setter 一律写不进去、读出来恒为 0
    try {
      Object.defineProperty(HTMLMediaElement.prototype, "volume", {
        get() {
          return 0;
        },
        set() {
          /* no-op：投屏窗口永远静音 */
        },
        configurable: true,
      });
    } catch {
      /* ignore */
    }

    // 每次 play 前强制静音（防 AudioAcrossFade 等组件淡入淡出后重新出声）
    const origPlay = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function (this: HTMLMediaElement) {
      this.muted = true;
      return origPlay.call(this);
    };

    // 监听动态挂载的 audio/video（角色语音、BGM、环境音等）
    const observer = new MutationObserver(() => {
      document.querySelectorAll("audio, video").forEach((el) => muteEl(el as HTMLMediaElement));
    });
    observer.observe(document.body, { childList: true, subtree: true });
    document.querySelectorAll("audio, video").forEach((el) => muteEl(el as HTMLMediaElement));
  }

  // ── 场景 / 角色应用（镜像与快照对账共用） ──────────────────

  // 背景图 / 背景效果
  function applyBackground(bg: string, effect: string) {
    if (bg && bg !== uiStore.currentBackground) uiStore.setCurrentBackground(bg);
    if (effect !== uiStore.currentBackgroundEffect) uiStore.setBackgroundEffect(effect);
  }

  // 场景光照（GameRolesStage 的 lightOverlayStyle 依赖 currentScene）
  async function applyScene(sceneId: string) {
    try {
      let scene = scenesCache.find((s) => s.id === String(sceneId));
      if (!scene) {
        scenesCache = await listScenes();
        scene = scenesCache.find((s) => s.id === String(sceneId));
      }
      if (scene && gameStore.currentScene?.id !== String(sceneId)) {
        gameStore.setCurrentScene(scene);
      }
    } catch {
      /* 场景光照缺失不阻塞渲染 */
    }
  }

  // 在场角色（有序 onstage 优先，后端已处理回退）：
  // 角色列表变化才重设，避免每次都打断 Live2D 舞台。
  async function applyRoles(roleIds: number[], currentRoleId: number | null) {
    const ids = roleIds ?? [];
    const curIds = gameStore.presentRoleIds;
    const rolesChanged = ids.length !== curIds.length || ids.some((id, i) => id !== curIds[i]);
    if (!rolesChanged) return;
    gameStore.presentRoleIds = [...ids];
    gameStore.mainRoleId = currentRoleId ?? ids[0] ?? -1;
    // 当前交互角色失联（被移下舞台）时回退到镜像角色；否则保持事件流给的值
    const curInteract = gameStore.currentInteractRoleId;
    if (curInteract == null || (ids.length > 0 && !ids.includes(curInteract))) {
      gameStore.currentInteractRoleId = currentRoleId ?? ids[0] ?? null;
    }
    await Promise.all(
      ids.map((id) =>
        gameStore
          .getOrCreateGameRole(id)
          .catch((e) => console.warn(`[Cast] 角色 ${id} 加载失败`, e))
      )
    );
  }

  // 当前角色的情绪状态写入角色对象：触发 Live2D 表情 / 静态立绘切换。
  // 主窗口由 DialogueProcessor 在台词出现时写 role.emotion / originalEmotion；
  // 投屏窗口不消费 ai:reply（否则按消息到达时间显示），只能由镜像携带的状态补上。
  async function applyRoleEmotion(roleId: number, emotion: string, originalEmotion: string) {
    try {
      const role = await gameStore.getOrCreateGameRole(roleId);
      const next = emotion || "正常";
      const nextOrig = originalEmotion || "正常";
      // 仅在变化时写，避免反复触发 Live2D 表情重放 / 立绘重新解析
      if (role.emotion !== next) role.emotion = next;
      if (role.originalEmotion !== nextOrig) role.originalEmotion = nextOrig;
    } catch (e) {
      console.warn(`[Cast] 同步角色 ${roleId} 情绪失败：`, e);
    }
  }

  // ── 镜像应用：主窗口当前显示什么，投屏就显示什么 ────────────
  async function applyMirror(m: CastMirror) {
    // 场景部分（仅变化时应用，避免打断 Live2D 舞台）
    applyBackground(m.background ?? "", m.backgroundEffect ?? "");
    if (m.currentSceneId) void applyScene(m.currentSceneId);
    await applyRoles(m.presentRoleIds ?? [], m.currentRoleId ?? null);

    // 情绪状态：在角色就位后应用到当前交互角色（Live2D 表情 / 立绘切换）
    if (m.currentRoleId != null) {
      await applyRoleEmotion(m.currentRoleId, m.roleEmotion ?? "", m.roleOriginalEmotion ?? "");
    }

    // 对话部分
    const line = m.line ?? "";
    uiStore.showCharacterLine = line;
    if (line) {
      // 台词非空 → 强制 'responding'：投屏只展示内容（输入框被 CSS 隐藏）。
      // 轮到玩家时主窗口是 'input'，若照搬状态，回复显示区会被 v-show 隐藏
      // （textarea 又被 CSS 藏掉）→ 一片空白。强制回应态让最后一句保持可见。
      uiStore.showCharacterTitle = m.title ?? "";
      uiStore.showCharacterSubtitle = m.subtitle ?? "";
      uiStore.showCharacterEmotion = m.emotion ?? "";
      uiStore.showCharacterMotionText = m.motionText ?? "";
      gameStore.currentStatus = "responding";
    } else {
      // 无台词（新会话 / 展示阶段清空）：回到闲置态，投屏显示空白
      gameStore.currentStatus = "input";
    }
  }

  // ── 快照对账（兜底）：背景/场景/角色 ────────────────────────
  // 台词完全由镜像驱动（保证与主界面逐句同步）；这里只兜底场景类状态——
  // 主窗口里用户本地点背景、选场景、改舞台角色等操作不广播事件也不改台词，
  // 定期拉后端权威快照（cast_get_snapshot）同步这些非对话状态。
  let reconciling = false;

  async function reconcileFromSnapshot() {
    if (reconciling) return;
    reconciling = true;
    try {
      let snap: CastSnapshot;
      try {
        snap = await invoke<CastSnapshot>("cast_get_snapshot");
      } catch (e) {
        console.warn("[Cast] 读取场景快照失败：", e);
        return;
      }

      applyBackground(snap.background ?? "", snap.backgroundEffect ?? "");
      if (snap.currentSceneId) await applyScene(String(snap.currentSceneId));
      await applyRoles(snap.presentRoleIds ?? [], snap.currentRoleId ?? null);
    } finally {
      reconciling = false;
    }
  }

  interface CastSnapshot {
    background: string | null;
    backgroundEffect: string | null;
    currentSceneId: string | null;
    presentRoleIds: number[];
    currentRoleId: number | null;
  }

  let reconcileTimer: ReturnType<typeof setInterval> | null = null;
  let unlistenMirror: UnlistenFn | null = null;
  let unlistenConfig: UnlistenFn | null = null;

  onMounted(async () => {
    applyCastMute();

    // 先注册实时镜像监听，再播种历史镜像，避免中间漏掉主窗口的台词推进
    unlistenMirror = await listen<CastMirror>("cast:mirror", (event) => {
      applyMirror(event.payload);
    });
    try {
      const stored = await invoke<CastMirror | null>("cast_get_mirror");
      if (stored) applyMirror(stored);
    } catch (e) {
      console.warn("[Cast] 读取投屏镜像失败：", e);
    }

    // 场景兜底对账（背景 / 场景 / 角色）
    await reconcileFromSnapshot();
    reconcileTimer = setInterval(reconcileFromSnapshot, 2000);

    // 调参初始值（窗口已打开时，设置页的改动经 cast:config 即时同步）
    try {
      const status = await invoke<Partial<CastTune>>("cast_get_status");
      applyTune(status);
    } catch {
      /* ignore */
    }

    // 设置页调整角色/对话框参数 → cast:config 事件即时生效（此处兜底读取仅在挂载/重开时）
    unlistenConfig = await listen<Partial<CastTune>>("cast:config", (event) => {
      applyTune(event.payload ?? {});
    });
  });

  onUnmounted(() => {
    if (reconcileTimer) clearInterval(reconcileTimer);
    unlistenMirror?.();
    unlistenConfig?.();
  });
</script>

<style scoped>
  .cast-shell {
    background: #000;
  }

  .cast-layer {
    position: absolute;
    inset: 0;
  }

  /* 角色舞台层：水平偏移经 CSS translateX 整层平移（与最初版本一致的已验证方案——
     水平平移不产生垂直裁剪，X 偏移不依赖布局链重跑即生效）。垂直偏移则折进
     角色布局（Live2DStage / GameRoleAvatar 的 castOffsetY，下移触底即止），
     避免整层下移把脚底锚点推出窗口被 .cast-shell overflow:hidden 裁断。 */
  .cast-role-layer {
    transform: translateX(var(--cast-char-offset-x, 0px));
  }

  /* 对话层：贴底居中（复刻主界面 .main-box 的 flex column + justify-end） */
  .cast-dialog-layer {
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
  }

  /* 无边框窗口拖拽条：顶部一条透明热区，按住可拖动投屏窗口 */
  .cast-drag-region {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 28px;
    z-index: 60;
    cursor: move;
    pointer-events: auto;
  }

  /* 投屏交互按钮：左上「日程」/ 右上「菜单」（与主界面 #menu-panel / FreeModeTools 同布局）。
     投屏窗口整体 pointer-events:none（全局样式），这里单独放开。
     z-index 对齐主界面：日程 2000、菜单 1000（均高于拖拽条 60）。 */
  .cast-top-left {
    position: fixed;
    top: calc(15px + var(--safe-area-inset-top, 0px));
    left: 20px;
    z-index: 2000;
    pointer-events: auto;
  }
  .cast-top-right {
    position: fixed;
    top: calc(15px + var(--safe-area-inset-top, 0px));
    right: 20px;
    z-index: 1000;
    pointer-events: auto;
  }
  /* 投屏窗口通常小于 xl 断点（1280px），强制显示「日程 / 菜单」按钮文字
     （主界面 hidden xl:block 在窄窗会只留图标） */
  .cast-top-left :deep(h3),
  .cast-top-right :deep(h3) {
    display: block !important;
  }
</style>

<style>
  /* 投屏窗口为被动展示：禁用一切交互（避免误触发送 / 关闭 / 改设置） */
  .cast-shell {
    pointer-events: none;
  }

  /* 隐藏对话框：整层对话不显示（投屏只展示角色舞台 / 背景）
     display:none 会连同打字机动画一起摘除，适合纯角色展示场景 */
  .cast-shell.cast-hide-dialog .cast-dialog-layer {
    display: none !important;
  }
  .cast-shell .cast-drag-region {
    pointer-events: auto;
  }

  /* 隐藏对话框的输入框、发送按钮、移动端折叠按钮与操作按钮组（投屏只展示内容） */
  .cast-shell #inputMessage,
  .cast-shell #sendButton,
  .cast-shell .mobile-toggle-btn,
  .cast-shell .mobile-menu-dropdown,
  .cast-shell .ml-auto {
    display: none !important;
  }

  /* ── 角色 / 对话框调参（设置页「投屏设置」滑块，cast:config 同步） ── */
  /* 角色层缩放由 cast-scale prop 注入角色布局（Live2D / 立绘）；水平偏移由 scoped 样式里
     .cast-role-layer translateX（整层平移）；垂直偏移由 cast-offset-y prop 折进角色布局。
     这里只保留对话框的覆盖规则。 */

  /* 对话框整体高度上限 + 超出裁剪：
     GameDialog 根容器默认 overflow 可见，长文会画出盒外、撑得容器看起来“错位”。
     限高 + overflow hidden 让内容在设定的对话框高度内被裁掉，不再外溢。 */
  .cast-shell .cast-dialog-layer > div {
    max-height: var(--cast-dialog-height, 40vh) !important;
    overflow: hidden !important;
  }
  /* 对话框内部宽度容器：控制左右的留白（100% = 顶满无留白） */
  .cast-shell .cast-dialog-layer > div > div {
    width: var(--cast-dialog-width, 70%) !important;
  }
  /* 对话框字体大小（主体 + 副标题 + 情绪标签 = 设定值；标题 ×1.2 保持主界面 24/20 比例） */
  .cast-shell .cast-dialog-layer .response-display,
  .cast-shell .cast-dialog-layer #character-sub,
  .cast-shell .cast-dialog-layer #character-emotion {
    font-size: var(--cast-dialog-font-size, 20px) !important;
  }
  .cast-shell .cast-dialog-layer #character {
    font-size: calc(var(--cast-dialog-font-size, 20px) * 1.2) !important;
  }
  /* 对话框背景色透明度（投屏独立覆盖，默认 70 = 复刻主界面 dialogOpacity 0.7 的
     渐变底色 #000e27）。覆盖内联的 dialogWrapperStyle 背景，并关掉模糊以保持一致。 */
  .cast-shell .cast-dialog-layer > div {
    background: linear-gradient(
      to top,
      rgba(0, 14, 39, var(--cast-dialog-bg-alpha, 0.7)),
      rgba(0, 14, 39, var(--cast-dialog-bg-alpha-top, 0.6))
    ) !important;
    backdrop-filter: none !important;
  }
</style>
