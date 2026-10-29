import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { eventQueue } from "../core/events/event-queue";
import type { ScriptEventType } from "../types";
import { useAdventureStore } from "../stores/modules/adventure";
import { useUIStore } from "../stores/modules/ui/ui";
import { useGameStore } from "../stores/modules/game";
import { useSettingsStore } from "../stores/modules/settings";
import { isWeatherEffect } from "@/components/game/standard/particles";
import { setWeatherAutoSync } from "./services/scene";
import { applyWebInitData } from "../stores/modules/game/actions";
import { i18n } from "@/locales";
import { useScriptEditorStore } from "../stores/modules/script-editor";
import {
  clearToolCallPreparing,
  handleToolActivity,
  handleToolCallProgress,
  interruptToolActivities,
  pushToolCallRecord,
  toolDisplayName,
  type EmbeddingAlertPayload,
  type ToolActivityEvent,
} from "./services/tool-settings";
import { useDialogStore } from "../stores/modules/ui/dialog";
import { useAsrStore } from "../stores/modules/settings/asr";
import type { VadEvent } from "../api/services/asr";
import type { SceneInfo } from "./services/scene";
import type { WebInitData } from "./services/game-info";
import { getAffection, type AffectionChangedPayload } from "./services/affection";
import { sendSystemNotification } from "@/composables/useSedentaryReminder";

function asEvent(
  payload: unknown,
  defaults: { type: string; defaultDuration: number; isFinal?: boolean },
): ScriptEventType {
  const p = payload as Record<string, unknown>;
  const duration = typeof p.duration === "number" ? p.duration : defaults.defaultDuration;
  return {
    ...p,
    type: defaults.type,
    duration,
    ...(defaults.isFinal !== undefined ? { isFinal: defaults.isFinal } : {}),
  } as unknown as ScriptEventType;
}

function isStalePreviewReply(payload: Record<string, unknown>): boolean {
  const gen = payload.previewGen;
  if (typeof gen !== "number") return false;
  const store = useScriptEditorStore();
  return !store.previewing || store.previewGeneration !== gen;
}

export function initializeTauriEventListeners() {
  const currentWindow = getCurrentWindow();
  const mainWindow = currentWindow.label === "main" ? currentWindow : null;

  listen("ai:reply", (event) => {
    const payload = event.payload as Record<string, unknown>;
    // 试玩中止后迟到的流式回复：直接丢弃，不放进事件队列
    if (isStalePreviewReply(payload)) return;
    console.log("[Tauri] 收到 ai:reply", event.payload);
    eventQueue.addEvent(asEvent(payload, { type: "reply", defaultDuration: -1 }));
  });

  listen("ai:thinking", (event) => {
    console.log("[Tauri] 收到 ai:thinking", event.payload);
    eventQueue.addEvent(asEvent(event.payload, { type: "thinking", defaultDuration: 0 }));
  });

  listen("ai:thinking_progress", (event) => {
    const payload = event.payload as { thinkingLength?: number };
    console.log("[Tauri] 收到 ai:thinking_progress", payload);
    const gameStore = useGameStore();
    if (typeof payload.thinkingLength === "number") {
      gameStore.thinkingLength = payload.thinkingLength;
    }
  });

  listen("ai:error", (event) => {
    const p = event.payload as Record<string, unknown>;
    console.log("[Tauri] 收到 ai:error", p);
    interruptToolActivities();
    eventQueue.addEvent({
      type: "error",
      duration: 0,
      error_code: (p.error_code as string) ?? "default_error",
      message: (p.detail as string) ?? "",
    } as ScriptEventType);
  });

  // 工具执行生命周期：驱动自由对话顶栏的实时状态，不写入历史记录。
  listen("ai:tool_activity", (event) => {
    const payload = event.payload as ToolActivityEvent;
    handleToolActivity(payload);
  });

  // 工具调用参数流式生成进度：顶栏实时显示「正在生成…N 字」
  listen("ai:tool_call_progress", (event) => {
    handleToolCallProgress(event.payload as { tool: string; chars: number });
  });

  // 一轮 LLM 流结束：清除「正在生成」进度提示（工具被忽略的收尾轮不会再有执行事件）
  listen("ai:tool_call_progress_end", () => {
    clearToolCallPreparing();
  });

  // 工具调用结果：记入「工具调用」页面历史 + 左上角弹通知
  listen("ai:tool_call", (event) => {
    const payload = event.payload as {
      tool: string;
      ok: boolean;
      summary: string;
      error: string | null;
      arguments: string;
      result: string;
    };
    pushToolCallRecord({
      ...payload,
      time: new Date().toLocaleTimeString(),
    });
    const toolLabel = toolDisplayName(payload.tool);
    const uiStore = useUIStore();
    if (payload.ok) {
      uiStore.showNotification({
        type: "success",
        title: i18n.global.t("ui.toolCalls.callSuccess"),
        message: `${toolLabel}：${payload.summary}`,
        duration: 3000,
        skipTipsCheck: true,
      });
    } else {
      uiStore.showNotification({
        type: "warning",
        title: i18n.global.t("ui.toolCalls.callFailed"),
        message: payload.error || toolLabel,
        duration: 4000,
        skipTipsCheck: true,
      });
    }
  });

  mainWindow?.listen("chat:command_approval", async (event) => {
    const payload = event.payload as {
      request_id: string;
      command: string;
      cwd: string;
      uac: boolean;
    };
    const dialogStore = useDialogStore();
    const message =
      i18n.global.t("ui.toolCalls.approvalMessage", {
        command: payload.command,
        cwd: payload.cwd || i18n.global.t("ui.toolCalls.approvalDefaultCwd"),
      }) + (payload.uac ? `\n\n${i18n.global.t("ui.toolCalls.approvalUac")}` : "");
    const approved = await dialogStore.confirm(
      message,
      i18n.global.t("ui.toolCalls.approvalTitle"),
    );
    try {
      await invoke("resolve_command_approval", { requestId: payload.request_id, approved });
    } catch (e) {
      console.warn("[Tauri] 回传命令审批结果失败（可能已超时）:", e);
    }
  });

  // execute_command 中识别到删除操作时使用独立危险确认；回传到删除审批队列。
  mainWindow?.listen("chat:command_delete_approval", async (event) => {
    const payload = event.payload as {
      request_id: string;
      command: string;
      cwd: string;
      uac: boolean;
    };
    const dialogStore = useDialogStore();
    const message =
      i18n.global.t("ui.toolCalls.commandDeleteApprovalMessage", {
        command: payload.command,
        cwd: payload.cwd || i18n.global.t("ui.toolCalls.approvalDefaultCwd"),
      }) + (payload.uac ? `\n\n${i18n.global.t("ui.toolCalls.approvalUac")}` : "");
    const approved = await dialogStore.confirm(
      message,
      i18n.global.t("ui.toolCalls.commandDeleteApprovalTitle"),
    );
    try {
      await invoke("resolve_file_delete_approval", {
        requestId: payload.request_id,
        approved,
      });
    } catch (e) {
      console.warn("[Tauri] 回传删除命令审批结果失败（可能已超时）:", e);
    }
  });

  // 逐次确认模式下，write_file / edit_file 在真正修改前显示目标路径。
  mainWindow?.listen("chat:file_change_approval", async (event) => {
    const payload = event.payload as {
      request_id: string;
      path: string;
      operation: "write" | "edit";
    };
    const dialogStore = useDialogStore();
    const approved = await dialogStore.confirm(
      i18n.global.t("ui.toolCalls.fileChangeApprovalMessage", {
        action: i18n.global.t(`ui.toolCalls.fileChangeActions.${payload.operation}`),
        path: payload.path,
      }),
      i18n.global.t("ui.toolCalls.fileChangeApprovalTitle"),
    );
    try {
      await invoke("resolve_file_change_approval", {
        requestId: payload.request_id,
        approved,
      });
    } catch (e) {
      console.warn("[Tauri] 回传文件修改审批结果失败（可能已超时）:", e);
    }
  });

  // 主聊天 delete_file 审批：先显示后端解析并校验过的真实路径，再把决定回传给工具。
  mainWindow?.listen("chat:file_delete_approval", async (event) => {
    const payload = event.payload as {
      request_id: string;
      path: string;
    };
    const dialogStore = useDialogStore();
    const approved = await dialogStore.confirm(
      i18n.global.t("ui.toolCalls.fileDeleteApprovalMessage", { path: payload.path }),
      i18n.global.t("ui.toolCalls.fileDeleteApprovalTitle"),
    );
    try {
      await invoke("resolve_file_delete_approval", {
        requestId: payload.request_id,
        approved,
      });
    } catch (e) {
      console.warn("[Tauri] 回传删除审批结果失败（可能已超时）:", e);
    }
  });

  listen("status:reset", (event) => {
    console.log("[Tauri] 收到 status:reset", event.payload);
    eventQueue.addEvent(asEvent(event.payload, { type: "status_reset", defaultDuration: 0 }));
  });

  listen("tts:cleanup", (event) => {
    const payload = event.payload as {
      deleted?: number;
      orphanFiles?: number;
      orphanSize?: number;
    };
    console.log("[Tauri] 收到 tts:cleanup", payload);
    try {
      localStorage.setItem(
        "lingchat:last_tts_cleanup",
        JSON.stringify({
          deleted: payload.deleted ?? 0,
          orphanFiles: payload.orphanFiles ?? 0,
          orphanSize: payload.orphanSize ?? 0,
          timestamp: Date.now(),
        }),
      );
    } catch (e) {
      console.warn("[Tauri] 保存 tts:cleanup 状态到 localStorage 失败:", e);
    }
  });

  // 待办截止提醒的「弹窗消息提醒」方式：后端只给结构化数据，文案在此按当前语言组装
  listen<{ title: string; todoText: string; minutesBefore: number }>(
    "proactive:remind",
    (event) => {
      const { title, todoText, minutesBefore } = event.payload;
      const body =
        minutesBefore >= 1440
          ? i18n.global.t("stores.proactiveReminder.daysLater", {
              text: todoText,
              count: Math.round(minutesBefore / 1440),
            })
          : minutesBefore >= 60
            ? i18n.global.t("stores.proactiveReminder.hoursLater", {
                text: todoText,
                count: Math.round(minutesBefore / 60),
              })
            : i18n.global.t("stores.proactiveReminder.minutesLater", {
                text: todoText,
                count: minutesBefore,
              });
      void sendSystemNotification(title, body);
    },
  );

  // 插件的对外连接（游戏桥接这类）连上了：给一句提示，省得玩家自己去插件页确认。
  // 只认女仆桥接插件，其余插件的连接不打扰界面。
  mainWindow?.listen<{ pluginId?: string; event?: string }>("plugin:ws-state", (event) => {
    const { pluginId, event: state } = event.payload ?? {};
    if (pluginId !== "mc_maid_bridge" || state !== "connect") return;
    useUIStore().showNotification({
      type: "success",
      title: i18n.global.t("ui.notification.mcMaidConnected"),
      skipTipsCheck: true,
    });
  });

  // === ASR events ===

  listen<VadEvent>("asr://speech_started", () => {
    useAsrStore().onSpeechStarted();
  });
  listen<VadEvent>("asr://turn_candidate", (event) => {
    useAsrStore().onTurnCandidate(event.payload);
  });
  listen<VadEvent>("asr://turn_sealed", () => {
    useAsrStore().onTurnSealed({ type: "turn_sealed" });
  });
  // 后端 init_asr VAD 模型加载成功（设置页状态面板显示"已加载"）
  listen("asr://vad_ready", () => {
    useAsrStore().setVadLoaded(true);
  });

  // === Adventure events ===

  listen("adventure:unlocked", (event) => {
    const payload = event.payload as any;
    console.log("[Tauri] 收到 adventure:unlocked", payload);
    const adventureStore = useAdventureStore();
    if (payload?.adventure_folder) {
      adventureStore.markAdventureUnlocked(payload.adventure_folder);
      adventureStore.unlockNotifications.push(payload);
    }
  });

  listen("adventure:completed", (event) => {
    const payload = event.payload as any;
    console.log("[Tauri] 收到 adventure:completed", payload);
    const adventureStore = useAdventureStore();
    if (payload?.adventure_folder) {
      adventureStore.markAdventureCompleted(payload.adventure_folder);
    }
  });

  // === Auto-save events ===

  listen("save:auto-saved", async (event) => {
    const payload = event.payload as { save_id: number; title: string; timestamp: string };
    console.log("[Tauri] 收到 save:auto-saved", payload);

    // Capture screenshot for auto-save slot
    const gameStore = useGameStore();
    // 自动存档会绑定 active_save_id，前端需同步以锁定身份切换
    if (typeof payload?.save_id === "number") {
      gameStore.activeSaveId = payload.save_id;
    }
    const screenshotPath = await gameStore.captureScreenshot();
    if (screenshotPath) {
      try {
        await invoke("save_screenshot", {
          saveId: payload.save_id,
          screenshotPath,
        });
      } catch (e) {
        console.error("[Tauri] 保存自动存档截图失败", e);
      }
    }

    useUIStore().showNotification({
      type: "info",
      title: i18n.global.t("api.events.autoSave.title"),
      message: i18n.global.t("api.events.autoSave.message", { time: payload.timestamp }),
      duration: 2500,
      skipTipsCheck: true,
    });
  });

  listen("save:auto-save-failed", (event) => {
    const payload = event.payload as { message: string; timestamp: string };
    console.log("[Tauri] 收到 save:auto-save-failed", payload);
    useUIStore().showNotification({
      type: "warning",
      title: i18n.global.t("api.events.autoSaveFailed.title"),
      message: i18n.global.t("api.events.autoSaveFailed.message", { time: payload.timestamp }),
      duration: 4000,
      skipTipsCheck: true,
    });
  });

  // === Affection events ===

  // 好感度评估结果（仅实际变化时发）：写回角色并记录最近一次变化
  listen("affection:changed", (event) => {
    const payload = event.payload as AffectionChangedPayload;
    console.log("[Tauri] 收到 affection:changed", payload);
    const gameStore = useGameStore();
    const role = gameStore.gameRoles[payload.role_id];
    if (role) {
      role.affection = payload.values;
      role.negative = payload.negative;
    }
    const deltaSum = Object.values(payload.deltas).reduce((sum, d) => sum + d, 0);
    gameStore.lastAffectionChange = {
      roleId: payload.role_id,
      deltaSum,
      deltas: payload.deltas,
      negativeDeltas: payload.negative_deltas,
      reason: payload.reason,
      at: Date.now(),
    };
  });

  // === Script events ===

  listen("script:narration", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "narration", defaultDuration: -1 }));
  });

  listen("script:player", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "player", defaultDuration: -1 }));
  });

  listen("script:chapter-change", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "chapter_change", defaultDuration: 0 }));
  });

  listen("script:background", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "background", defaultDuration: 0 }));
  });

  listen("script:background-effect", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "background_effect", defaultDuration: 0 }));
  });

  listen("script:music", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "music", defaultDuration: 0 }));
  });

  listen("script:sound", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "sound", defaultDuration: 0 }));
  });

  // 环境音事件（多轨并行，与BGM共存）
  listen("script:ambient", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "ambient", defaultDuration: 0 }));
  });

  listen("script:present-pic", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "present_pic", defaultDuration: -1 }));
  });

  listen("script:modify-character", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "modify_character", defaultDuration: 0 }));
  });

  listen("script:input", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "input", defaultDuration: 0 }));
  });

  listen("script:choice", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "choice", defaultDuration: 0 }));
  });

  listen("script:progress", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "progress", defaultDuration: 0 }));
  });

  listen("script:end", (event) => {
    console.log("[Tauri] 收到 script:end", event.payload);
    eventQueue.addEvent(
      asEvent(event.payload, { type: "script_end", defaultDuration: 0, isFinal: true }),
    );
  });

  listen("script:free-dialogue", (event) => {
    eventQueue.addEvent(asEvent(event.payload, { type: "free_dialogue", defaultDuration: 0 }));
  });

  // === God Agent multi-dialogue event ===

  listen("character:switch", async (event) => {
    const payload = event.payload as { type: string; roleId: number; characterName: string };
    console.log("[Tauri] 收到 character:switch", payload);
    const gameStore = useGameStore();
    const uiStore = useUIStore();
    // 先确保角色数据已加载（立绘/名字都从这里取）
    const role = await gameStore.getOrCreateGameRole(payload.roleId);
    gameStore.currentInteractRoleId = payload.roleId;
    if (!gameStore.presentRoleIds.includes(payload.roleId)) {
      gameStore.presentRoleIds = [payload.roleId];
    }
    // 同步主界面/桌宠标题（对话中名字由 currentInteractRole 驱动，已覆盖）
    uiStore.showCharacterTitle = role.roleName;
    uiStore.showCharacterSubtitle = role.roleSubTitle;
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
      console.warn("[Affection] 切换角色后刷新好感度失败:", e);
    }
  });

  // === LLM 工具事件（场景 / 换装） ===

  // ambient:effect 按来源落层：
  //   layer=weather —— 天气自动同步，只写天气层
  //   source=manual —— 设置页/桌宠手动选择，只写对应层
  //   其余为 AI 工具调用，只写对应层；动了天气层时关闭天气自动同步
  listen("ambient:effect", (event) => {
    const payload = event.payload as {
      type: string;
      effect: string;
      duration?: number;
      layer?: string;
      source?: string;
    };
    console.log("[Tauri] 收到 ambient:effect", payload);
    const settings = useSettingsStore();
    if (payload.source === "manual") {
      if (isWeatherEffect(payload.effect)) {
        settings.setWeatherEffect(payload.effect);
      } else {
        settings.setBackgroundEffect(payload.effect);
      }
      return;
    }
    if (payload.layer === "weather") {
      settings.setWeatherEffect(payload.effect);
      return;
    }
    // AI 动了天气层 → 天气同步自动关闭；none 清除两层
    if (isWeatherEffect(payload.effect) || payload.effect === "None") {
      setWeatherAutoSync(false).catch((e) => console.warn("setWeatherAutoSync 失败:", e));
    }
    if (isWeatherEffect(payload.effect)) {
      settings.setWeatherEffect(payload.effect);
    } else if (payload.effect === "None") {
      settings.setBackgroundEffect("None");
      settings.setWeatherEffect("None");
    } else {
      settings.setBackgroundEffect(payload.effect);
    }
  });

  listen("scene:switch", (event) => {
    const payload = event.payload as { type: string; scene: SceneInfo };
    console.log("[Tauri] 收到 scene:switch", payload);
    const gameStore = useGameStore();
    const uiStore = useUIStore();
    gameStore.setCurrentScene(payload.scene);
    uiStore.setCurrentBackground(payload.scene.background ?? "");
  });

  listen("character:clothes-changed", (event) => {
    const payload = event.payload as { roleId: number; clothesName: string };
    console.log("[Tauri] 收到 character:clothes-changed", payload);
    const role = useGameStore().gameRoles[payload.roleId];
    if (role) role.clothesName = payload.clothesName;
  });

  listen("character:full-switch", (event) => {
    console.log("[Tauri] 收到 character:full-switch", event.payload);
    applyWebInitData(useGameStore().$state, event.payload as WebInitData);
    eventQueue.clear();
    eventQueue.resume();
  });

  // 插件请求切换「自动模式」（plugin_host.set_auto_mode）：插件接上外部程序后会把主界面
  // 切进自动逐句推进——玩家在那边（游戏里、投屏端）腾不出手点对话框。该模式不落盘，
  // 也不会因为插件断开而自动还原，插件要还原得自己再请求一次。
  listen<{ enabled: boolean }>("plugin:auto-mode", (event) => {
    useUIStore().autoMode = !!event.payload?.enabled;
  });

  // 云端向量化熔断：本轮及后续检索不再做向量化（笔记全量返回、工具全量下发），
  // 让用户知道原因与恢复方式，避免误以为是检索本身坏了。
  listen<EmbeddingAlertPayload>("embedding:alert", (event) => {
    const payload = event.payload;
    console.warn("[Tauri] 收到 embedding:alert", payload);
    useUIStore().showNotification({
      type: "warning",
      title: i18n.global.t("ui.toolCalls.embedding.alertTitle"),
      message: i18n.global.t("ui.toolCalls.embedding.alertMessage", {
        backend: payload?.backend ?? "cloud",
        reason: payload?.reason ?? "",
      }),
      duration: 10000,
      skipTipsCheck: true,
    });
  });

  console.log(
    "[Tauri] 事件监听已注册（ai + ai:thinking_progress + tts:cleanup + adventure + 自动存档 + affection:changed + 13 个剧本事件 + character:switch + scene:switch + character:clothes-changed + character:full-switch）",
  );
}

export function initializeCastWindowListeners() {
  listen("character:switch", async (event) => {
    const payload = event.payload as { type: string; roleId: number; characterName: string };
    const gameStore = useGameStore();
    const uiStore = useUIStore();
    const role = await gameStore.getOrCreateGameRole(payload.roleId);
    gameStore.currentInteractRoleId = payload.roleId;
    if (!gameStore.presentRoleIds.includes(payload.roleId)) {
      gameStore.presentRoleIds = [payload.roleId];
    }
    uiStore.showCharacterTitle = role.roleName;
    uiStore.showCharacterSubtitle = role.roleSubTitle;
  });

  // 与主窗口同一份处理：投屏窗口的 store 是独立实例，必须自己接一次
  listen("character:clothes-changed", (event) => {
    const payload = event.payload as { roleId: number; clothesName: string };
    const role = useGameStore().gameRoles[payload.roleId];
    if (role) role.clothesName = payload.clothesName;
  });

  listen("scene:switch", (event) => {
    const payload = event.payload as { type: string; scene: SceneInfo };
    const gameStore = useGameStore();
    const uiStore = useUIStore();
    gameStore.setCurrentScene(payload.scene);
    uiStore.setCurrentBackground(payload.scene.background ?? "");
  });

  // 投屏客户端麦克风经投屏 /ws 送到 Rust ASR，识别文本由这里注入对话。
  listen("cast:mic:recognized", (event) => {
    const { text } = event.payload as { text: string };
    if (!text) return;
    window.dispatchEvent(new CustomEvent("asr-send", { detail: text }));
  });

  console.log(
    "[Tauri] 投屏窗口事件监听已注册（scene:switch + character:switch + character:clothes-changed + cast:mic:recognized）",
  );
}
