// actions.ts
import type { GameState, GameMessage, GameRole } from "./state";
import { getGameInfo } from "../../../api/services/game-info";
import type { GameLineInit, WebInitData } from "../../../api/services/game-info";
import { getRoleInfo } from "../../../api/services/character";
import { useUIStore } from "../ui/ui";
import { useSettingsStore } from "../settings";
import type { SceneInfo } from "@/api/services/scene";
import { invoke } from "@tauri-apps/api/core";
import { resetScriptWindowTitle } from "@/utils/windowTitleCoordinator";

function clearStoryInteractionState(state: GameState) {
  state.runningScript = null;
  state.forceChoice = null;
  state.poemGame = null;
}

function restoreStoryMedia(uiStore: ReturnType<typeof useUIStore>) {
  if (uiStore.preScriptBgm !== null) {
    uiStore.currentBackgroundMusic = uiStore.preScriptBgm;
    uiStore.preScriptBgm = null;
  }
  if (uiStore.preScriptBgmMode !== null) {
    uiStore.bgMusicMode = uiStore.preScriptBgmMode;
    uiStore.preScriptBgmMode = null;
  }
  uiStore.bgmPersistBlocked = false;
  uiStore.clearAmbientTracks();
  uiStore.triggerSoundEffect("None");
  uiStore.bgMusicPlaybackRate = 1;
}

function clearStoryEffects(uiStore: ReturnType<typeof useUIStore>) {
  uiStore.resetHorrorEffects();
  resetScriptWindowTitle();
  uiStore.showPlayerHintLine = "";
}

function restoreStoryStage(state: GameState, uiStore: ReturnType<typeof useUIStore>) {
  if (uiStore.preScriptBackground !== null) {
    useSettingsStore().setCurrentBackground(uiStore.preScriptBackground);
    uiStore.preScriptBackground = null;
  }
  if (state.preScriptRoleIds !== null) {
    state.presentRoleIds = state.preScriptRoleIds;
    state.preScriptRoleIds = null;
  }
  for (const id of state.presentRoleIds) {
    const role = state.gameRoles[id];
    if (role) {
      role.show = true;
      role.emotion = "正常";
    }
  }
}

function releaseStorySystemResources(notifyBackend: boolean) {
  invoke("close_script_glitch_windows").catch((err) =>
    console.warn("[Script] 关闭故障窗口失败（非致命）:", err)
  );
  if (notifyBackend) {
    invoke("stop_script").catch((err) => console.warn("[Script] stop_script 失败（非致命）:", err));
  }
}

export const actions = {
  appendGameMessage(this: GameState, message: GameMessage) {
    this.dialogHistory.push({
      ...message,
      timestamp: Date.now(),
    });
  },

  setGameMessages(this: GameState, messages: GameMessage[]) {
    this.dialogHistory = messages;
  },

  async initializeGame(this: GameState) {
    try {
      const gameInfo = await getGameInfo();
      applyWebInitData(this, gameInfo);
      // 剧本入口会先标记 runningScript 再挂载聊天页。initializeGame 的
      // backend snapshot 仍可能带回自由对话角色，所以必须在事件队列 resume
      // 之前再次清台；随后 authored show_character 事件会按顺序重建舞台。
      if (this.runningScript) {
        this.presentRoleIds = [];
      } else {
        invoke("notify_player_entry").catch((err) =>
          console.warn("[Entry] 问候触发失败（非致命）:", err)
        );
      }
      return gameInfo;
    } catch (error) {
      console.error("初始化游戏信息失败:", error);
      throw error;
    }
  },

  async getOrCreateGameRole(this: GameState, role_id: number): Promise<GameRole> {
    if (this.gameRoles[role_id]) {
      return this.gameRoles[role_id];
    }
    try {
      const roleInfo = await getRoleInfo(role_id);
      this.gameRoles[role_id] = {
        roleId: roleInfo.character_id,
        roleName: roleInfo.ai_name,
        roleSubTitle: roleInfo.ai_subtitle,
        thinkMessage: roleInfo.thinking_message,
        scale: roleInfo.scale,
        offsetX: roleInfo.offset_x,
        offsetY: roleInfo.offset_y,
        scaleP: roleInfo.scale_p,
        offsetXP: roleInfo.offset_x_p,
        offsetYP: roleInfo.offset_y_p,
        bubbleLeft: roleInfo.bubble_left,
        bubbleTop: roleInfo.bubble_top,
        clothes: roleInfo.clothes,
        clothesName: roleInfo.clothes_name,
        bodyPart: roleInfo.body_part,
        live2d: roleInfo.live2d,
        character_folder: roleInfo.character_folder,
        emotion: "正常",
        originalEmotion: "正常",
        show: true,
      };
      return this.gameRoles[role_id];
    } catch (error) {
      console.error("游戏角色信息获取失败:", error);
      throw error;
    }
  },

  /** 标记进入剧情模式（用于控制UI显示：隐藏番茄钟/日程等） */
  enterStoryMode(
    this: GameState,
    scriptName: string = "unknown",
    contentWarning?: string,
    folderKey?: string
  ) {
    this.poemGame = null;
    this.runningScript = {
      scriptName,
      folderKey,
      currentChapterName: "",
      choices: [],
      isRunning: true,
      freeDialogueInfo: {
        isFreeDialogue: false,
        maxRounds: -1,
        currentRound: 0,
        endLine: "",
      },
      contentWarning,
    };
    const uiStore = useUIStore();
    // 剧本 BGM 隔离：保存自由对话的 BGM 与循环模式（已在剧本中则保留最早的值），
    // 并阻止剧本期间的 BGM 变化被持久化到 session（防泄漏到下次启动）
    if (uiStore.preScriptBgm === null) {
      uiStore.preScriptBgm = uiStore.currentBackgroundMusic;
      uiStore.preScriptBgmMode = uiStore.bgMusicMode;
    }
    // 背景图同理：剧本换的崩坏背景不得残留到自由对话
    if (uiStore.preScriptBackground === null) {
      uiStore.preScriptBackground = useSettingsStore().display.currentBackground;
    }
    // 在场角色快照：剧本演出的 hide_character（结局"角色消失"）会改写
    // presentRoleIds，退出时必须恢复，否则自由对话立绘消失
    if (this.preScriptRoleIds === null) {
      this.preScriptRoleIds = [...this.presentRoleIds];
    }
    // Every story run owns a deterministic stage. Free-dialogue characters must
    // not leak into missing/empty scenes; the snapshot is restored on exit.
    this.presentRoleIds = [];
    uiStore.bgmPersistBlocked = true;
    uiStore.bgMusicMode = "loop-single";
    // 进入新剧本前清掉可能残留的恐怖特效（上次异常退出/重启等情况）
    uiStore.resetHorrorEffects();
  },

  /** 标记退出剧情模式，回到自由对话模式；各子步骤均为幂等 helper。 */
  exitStoryMode(this: GameState, notifyBackend = true) {
    const uiStore = useUIStore();
    clearStoryInteractionState(this);
    restoreStoryMedia(uiStore);
    clearStoryEffects(uiStore);
    restoreStoryStage(this, uiStore);
    releaseStorySystemResources(notifyBackend);
  },

  // 设置当前场景（仅更新 store，不调用 API）
  setCurrentScene(this: GameState, scene: SceneInfo | null) {
    this.currentScene = scene;
  },

  /** 标记 LoadingTransition 启动动画已完成（§1.9 门控：动画期间不启动 ASR） */
  setLoadingComplete(this: GameState, v: boolean) {
    this.loadingComplete = v;
  },

  // 清除场景（更新 store，API 调用由组件负责）
  clearCurrentScene(this: GameState) {
    this.currentScene = null;
  },

  /** 截图主窗口（1 次 IPC，0 次窗口枚举）。若已有截图进行中则复用同一个 Promise。 */
  async captureScreenshot(this: GameState): Promise<string | null> {
    // 已有截图进行中 → 复用
    if (this.screenshotPending) return this.screenshotPending;

    this.screenshotPending = (async () => {
      try {
        const filePath = await invoke<string>("capture_main_window_screenshot");
        if (!filePath) {
          console.warn("[Screenshot] capture_main_window_screenshot returned empty path");
          return null;
        }
        console.log("[Screenshot] Captured:", filePath);
        this.latestScreenshot = filePath;
        return filePath;
      } catch (err) {
        console.error("[Screenshot] Capture failed:", err);
        return null;
      } finally {
        this.screenshotPending = null;
      }
    })();

    return this.screenshotPending;
  },
};

/** 将 WebInitData 写入 GameState（init / 角色切换共用） */
export function applyWebInitData(state: GameState, gameInfo: WebInitData): void {
  const characterInfo = gameInfo.character_settings;
  const charId = characterInfo.character_id ?? 0;

  // 从 onstage_roles 填充 gameRoles（含主角 + 所有在场角色）
  state.gameRoles = {};
  for (const settings of gameInfo.onstage_roles) {
    const rid = settings.character_id ?? 0;
    if (rid === 0) continue;
    state.gameRoles[rid] = {
      roleId: rid,
      roleName: settings.ai_name,
      roleSubTitle: settings.ai_subtitle,
      thinkMessage: settings.thinking_message,
      scale: settings.scale,
      offsetX: settings.offset_x,
      offsetY: settings.offset_y,
      scaleP: settings.scale_p,
      offsetXP: settings.offset_x_p,
      offsetYP: settings.offset_y_p,
      bubbleLeft: settings.bubble_left,
      bubbleTop: settings.bubble_top,
      clothes: settings.clothes,
      clothesName: settings.clothes_name,
      bodyPart: settings.body_part,
      live2d: settings.live2d,
      character_folder: settings.character_folder,
      emotion: "正常",
      originalEmotion: "正常",
      show: true,
    };
  }

  // fallback：若 onstage_roles 中未包含主角（如旧版存档），从 character_settings 补充
  if (!state.gameRoles[charId] && charId !== 0) {
    state.gameRoles[charId] = {
      roleId: charId,
      roleName: characterInfo.ai_name,
      roleSubTitle: characterInfo.ai_subtitle,
      thinkMessage: characterInfo.thinking_message,
      scale: characterInfo.scale,
      offsetX: characterInfo.offset_x,
      offsetY: characterInfo.offset_y,
      scaleP: characterInfo.scale_p,
      offsetXP: characterInfo.offset_x_p,
      offsetYP: characterInfo.offset_y_p,
      bubbleLeft: characterInfo.bubble_left,
      bubbleTop: characterInfo.bubble_top,
      clothes: characterInfo.clothes,
      clothesName: characterInfo.clothes_name,
      bodyPart: characterInfo.body_part,
      live2d: characterInfo.live2d,
      character_folder: characterInfo.character_folder,
      emotion: "正常",
      originalEmotion: "正常",
      show: true,
    };
  }

  state.presentRoleIds =
    gameInfo.onstage_roles_ids.length > 0 ? [...gameInfo.onstage_roles_ids] : [charId];
  state.mainRoleId = charId;
  state.currentInteractRoleId = gameInfo.current_interact_role_id ?? charId;

  const uiStore = useUIStore();
  const settingsStore = useSettingsStore();
  state.userName = characterInfo.user_name;
  state.userSubtitle = characterInfo.user_subtitle;

  uiStore.showCharacterTitle = characterInfo.ai_name;
  uiStore.showCharacterSubtitle = characterInfo.ai_subtitle;

  if (gameInfo.background !== "") uiStore.setCurrentBackground(gameInfo.background);
  if (gameInfo.background_effect !== "") uiStore.setBackgroundEffect(gameInfo.background_effect);

  // 恢复背景音乐：用户上次手动选择优先于场景/剧本设定
  if (gameInfo.last_bgm_track && gameInfo.last_bgm_track !== "None") {
    uiStore.currentBackgroundMusic = gameInfo.last_bgm_track;
  } else if (gameInfo.background_music !== "") {
    uiStore.currentBackgroundMusic = gameInfo.background_music;
  }
  if (gameInfo.last_bgm_paused != null) {
    uiStore.bgMusicPaused = gameInfo.last_bgm_paused;
  }
  if (gameInfo.last_bgm_mode) {
    uiStore.bgMusicMode = gameInfo.last_bgm_mode as "loop-single" | "loop-list" | "random";
  }

  // 恢复环境音轨道（标记为暂停，避免启动时自动播放）
  if (gameInfo.last_ambient_tracks) {
    try {
      const tracks = JSON.parse(gameInfo.last_ambient_tracks);
      if (Array.isArray(tracks) && tracks.length > 0) {
        uiStore.ambientTracks = tracks.map((t: any) => ({ ...t, paused: true }));
      }
    } catch (e) {
      console.warn("解析环境音轨道数据失败:", e);
    }
  }

  // 同步场景感知开关
  settingsStore.setSceneAwarenessEnabled(gameInfo.scene_awareness_enabled);

  // 恢复场景状态
  if (gameInfo.current_scene) {
    state.currentScene = gameInfo.current_scene;
  }

  if (gameInfo.lines && gameInfo.lines.length > 0) {
    state.dialogHistory = convertInitLines(gameInfo.lines);
  } else {
    state.dialogHistory = [];
  }

  state.initialized = true;
}

/** 将 Rust GameLineInit 转换为前端 GameMessage 列表 */
export function convertInitLines(lines: GameLineInit[]): GameMessage[] {
  const filtered = lines.filter((line) => line.attribute !== "system" && line.attribute !== "tool");

  return filtered.map((line, index, array) => {
    const filteredContent = line.content.replace(/\{[\s\S]*?\}/g, "").trim();

    const isLast = index === array.length - 1;
    const nextLine = isLast ? null : array[index + 1];
    let isFinal = false;
    if (line.attribute === "assistant") {
      if (isLast || nextLine?.attribute === "user") {
        isFinal = true;
      }
    }

    return {
      type: (line.attribute === "user" ? "message" : "reply") as "message" | "reply",
      displayName: line.display_name || "",
      content: filteredContent,
      emotion: line.predicted_emotion || undefined,
      audioFile: line.audio_file || undefined,
      isFinal,
      motionText: line.action_content || undefined,
      originalTag: line.original_emotion || undefined,
      timestamp: Date.now(),
      userMessageSeq: line.user_message_seq ?? undefined,
      thinking: line.thinking || undefined,
      ttsText: line.tts_content || undefined,
      senderRoleId: line.sender_role_id,
    };
  });
}
