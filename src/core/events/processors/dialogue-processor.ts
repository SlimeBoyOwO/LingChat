import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { IEventProcessor } from "../event-processor";
import type { ScriptDialogueEvent } from "../../../types";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { isJaLocale, hkify } from "@/locales";
import type { GameMessage, GameRole } from "@/stores/modules/game/state";

// 只有主窗口在消费对话事件队列（其余窗口即便注册了 ai:reply 也不会 resume），
// 「这一句上屏了」的外部通知只由它发出，否则外部会收到重复的同一句。
const isMainWindow = getCurrentWindow().label === "main";

export default class DialogueProcessor implements IEventProcessor {
  private lastReply?: GameMessage;
  private emotionRepeats = new WeakMap<GameRole, { emotion: string; count: number }>();

  canHandle(eventType: string): boolean {
    return eventType === "reply";
  }

  async processEvent(event: ScriptDialogueEvent): Promise<void> {
    const gameStore = useGameStore();
    const uiStore = useUIStore();

    // 更新游戏状态显示对话
    gameStore.currentStatus = "responding";

    // 针对剧本模式，获取角色
    const role = await gameStore.getOrCreateGameRole(event.roleId);
    if (!role) {
      console.warn("角色修改的角色似乎并没有被初始化");
      return;
    }

    // 用实际展示的历史边界区分回复，不依赖可能被队列跳过的 thinking 事件。
    // 玩家新消息、历史被替换、上一轮结束或消息序号变化，都重新计算情绪次数。
    const previousMessage = gameStore.dialogHistory[gameStore.dialogHistory.length - 1];
    const newReply =
      !previousMessage ||
      previousMessage !== this.lastReply ||
      previousMessage.isFinal === true ||
      (typeof event.userMessageSeq === "number" &&
        event.userMessageSeq !== previousMessage.userMessageSeq);
    if (newReply) this.emotionRepeats = new WeakMap();

    const displayName = event.displayName ? event.displayName : role.roleName;
    const displaySubtitle = event.displaySubtitle ? event.displaySubtitle : role.roleSubTitle;

    // 日文界面且存在日语译文时显示日语译文；繁体（香港）界面下对话转繁体显示
    const displayLine = hkify(isJaLocale() && event.ttsText ? event.ttsText : event.message || "");
    gameStore.currentLine = displayLine;
    uiStore.showCharacterMotionText = event.motionText || "";

    gameStore.appendGameMessage({
      type: "reply",
      displayName: displayName,
      content: event.message,
      emotion: event.emotion,
      audioFile: event.audioFile,
      isFinal: event.isFinal,
      motionText: event.motionText,
      originalTag: event.originalTag,
      userMessageSeq: event.userMessageSeq,
      thinking: event.thinking,
      ttsText: event.ttsText,
      senderRoleId: event.roleId,
    });
    this.lastReply = gameStore.dialogHistory[gameStore.dialogHistory.length - 1];

    // 回溯更新最近一条没有序号标记的用户消息（前端发送消息时尚未拿到序号）
    if (typeof event.userMessageSeq === "number") {
      const history = gameStore.dialogHistory;
      for (let i = history.length - 1; i >= 0; i--) {
        if (history[i].type === "message" && history[i].userMessageSeq === undefined) {
          history[i].userMessageSeq = event.userMessageSeq;
          break;
        }
      }
    }

    uiStore.showCharacterLine = gameStore.currentLine; // TODO: 这部分逻辑之后整合
    // 告诉插件「这一句上屏了」：外部渲染（游戏内气泡、字幕、投屏）据此跟主界面的节奏
    // 对齐，而不是跟着模型的生成时间一次涌出去。事件原样转发，插件侧按 previewGen
    // 过滤试玩流。空台词的收尾 reply 没有可展示内容，不问。
    if (isMainWindow && (event.message ?? "").trim()) {
      invoke("plugin_line_shown", { line: event }).catch(() => {});
    }
    // 空收尾只标记回复结束，不计入同情绪次数，也不改变当前立绘。
    if (String(event.message ?? "").trim() || String(event.motionText ?? "").trim()) {
      const emotion = event.emotion || "正常";
      const previous = this.emotionRepeats.get(role);
      const count =
        role.emotion === emotion && previous?.emotion === emotion ? previous.count + 1 : 1;
      // 同一回复中按角色计数：第一次播放、第二次跳过、第三次重播，之后隔次播放。
      this.emotionRepeats.set(role, { emotion, count: count === 3 ? 1 : count });
      role.emotion = emotion;
      role.originalEmotion = event.originalTag || "正常";
      if (count !== 2) role.emotionRevision = (role.emotionRevision ?? 0) + 1;
    }
    gameStore.currentInteractRoleId = role.roleId;
    uiStore.currentAvatarAudio = event.audioFile || "None";
    // 前端触发对话/播放回复音频时，把该句语音广播给投屏客户端（远端设备同步播放）。
    // 仅主窗口处理 ai:reply 事件，这里每句回复恰好执行一次；投屏服务未运行时命令内 no-op。
    if (event.audioFile) {
      invoke("cast_play_voice", { audioFile: event.audioFile }).catch(() => {});
    }
    uiStore.showCharacterEmotion = role.originalEmotion;

    uiStore.showCharacterTitle = displayName;
    uiStore.showCharacterSubtitle = displaySubtitle;
    // gameStore.currentCharacter = event.character;

    // 对话总是等待用户继续，所以这里不需要做任何等待
    // event-queue 会自动检测到这是对话事件并等待用户继续
  }
}
