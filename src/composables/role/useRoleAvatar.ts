/**
 * 角色头像解析与情绪演出 —— 主界面（game/standard）与桌宠（pet）共用。
 *
 * 此前两边的 GameRoleAvatar.vue 各有一份近乎逐字的实现，两块逻辑：
 *   1. 解析头像文件（get_avatar_file + 归一化 + 竞态保护）
 *   2. 由 role.emotion / gameStore.currentStatus 驱动动画类、气泡图、气泡音效
 *
 * 两边刻意的差异已按评审结论收敛成统一行为，因此本文件不含模式分支：
 *   - 气泡图**不带** `?t=…` cache-buster（采桌宠实现：加了会因本地重载疯狂闪烁）
 *   - 情绪气泡显示前先置 false → nextTick → true 重触发过渡（采主界面实现：
 *     去掉 cache-buster 后这是唯一的重播手段，连续两次同情绪也必须重播）
 *
 * 真正还需要模式区分的只剩「等谁加载完」——主界面是 Live2D/静态呈现层，
 * 桌宠是 ImageCrossFade ——由 waitForAvatarLoad 注入。
 */
import { ref, watch, nextTick, onUnmounted, type Ref } from "vue";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { EMOTION_CONFIG, EMOTION_CONFIG_EMO } from "@/controllers/emotion/config";
import type { GameRole } from "@/stores/modules/game/state";

/** 气泡默认停留时长（毫秒） */
const DEFAULT_BUBBLE_MS = 2000;

export interface UseRoleAvatarOptions {
  role: Ref<GameRole>;
  /** 模板里的 `<audio ref>`，用于播放情绪气泡音效 */
  audioRef: Ref<HTMLAudioElement | null>;
  /**
   * 等待展示层就绪（图片/模型加载完成）后再放动画。
   * 主界面传 presentationRef.waitForLoad()，桌宠传 imageFadeRef.waitForLoad()。
   */
  waitForAvatarLoad?: () => Promise<void> | void;
  /** 气泡停留时长，默认 2000ms */
  bubbleDurationMs?: number;
}

export interface UseRoleAvatarApi {
  /** 当前情绪对应的头像图片 URL（已 convertFileSrc） */
  targetAvatarUrl: Ref<string>;
  /** 待机/情绪动画类名，如 "normal" / "happy-bounce" */
  activeAnimationClass: Ref<string>;
  isBubbleVisible: Ref<boolean>;
  currentBubbleImageUrl: Ref<string>;
  currentBubbleClass: Ref<string>;
  resolveAvatar: () => Promise<void>;
  /** 绑定到动画元素的 @animationend */
  handleAnimationEnd: () => void;
}

/**
 * 解析头像文件前的归一化（纯函数，便于核对映射）：
 * 未指定衣服回落 default，表外情绪回落「正常」。
 */
export function avatarFolderParams(role: GameRole): {
  characterFolder: string;
  emotion: string;
  clothesName: string;
} {
  return {
    characterFolder: role.character_folder,
    emotion: EMOTION_CONFIG_EMO[role.emotion] || "正常",
    clothesName: role.clothesName === "默认" || !role.clothesName ? "default" : role.clothesName,
  };
}

export function useRoleAvatar(options: UseRoleAvatarOptions): UseRoleAvatarApi {
  const { role, audioRef } = options;
  const bubbleDurationMs = options.bubbleDurationMs ?? DEFAULT_BUBBLE_MS;

  const gameStore = useGameStore();
  const uiStore = useUIStore();

  const targetAvatarUrl = ref("");
  const activeAnimationClass = ref("normal");
  const isBubbleVisible = ref(false);
  const currentBubbleImageUrl = ref("");
  const currentBubbleClass = ref("");

  let bubbleTimeoutId: number | null = null;
  // 两个代次号各管一条异步链：头像解析、情绪演出
  let resolveAvatarId = 0;
  let latestEmotionId = 0;

  async function resolveAvatar() {
    const currentId = ++resolveAvatarId;
    try {
      const path = await invoke<string>("get_avatar_file", avatarFolderParams(role.value));
      if (currentId === resolveAvatarId) {
        targetAvatarUrl.value = convertFileSrc(path);
      }
    } catch {
      if (currentId === resolveAvatarId) {
        targetAvatarUrl.value = "";
      }
    }
  }

  // 播放情绪气泡音效（音量跟随「气泡音量」设置，否则恒为满音量）
  const playBubbleAudio = (src: string) => {
    if (!audioRef.value) return;
    audioRef.value.volume = uiStore.bubbleVolume / 100;
    audioRef.value.src = src;
    audioRef.value.load();
    audioRef.value.play().catch((e) => console.error("气泡音效播放失败:", e));
  };

  const clearBubbleTimer = () => {
    if (bubbleTimeoutId !== null) {
      window.clearTimeout(bubbleTimeoutId);
      bubbleTimeoutId = null;
    }
  };

  /** 安排气泡在停留时长后自动隐藏 */
  const armBubbleHide = () => {
    clearBubbleTimer();
    bubbleTimeoutId = window.setTimeout(() => {
      isBubbleVisible.value = false;
      bubbleTimeoutId = null;
    }, bubbleDurationMs);
  };

  /**
   * 情绪气泡：先置 false → nextTick 置 true，重触发 CSS 过渡。
   * 连续两次相同情绪时也要重播（两次的图片 URL 相同，光换样式不会重播）。
   */
  const showEmotionBubble = (image: string, className: string) => {
    currentBubbleImageUrl.value = image;
    currentBubbleClass.value = className;
    isBubbleVisible.value = false;
    nextTick(() => {
      isBubbleVisible.value = true;
      armBubbleHide();
    });
  };

  /**
   * 思考气泡：不重触发。currentStatus 离开 thinking 时下面的 watch 已先隐藏，
   * 再进 thinking 必然是 false → true，天然会重播。与重构前行为一致。
   */
  const showThinkingBubble = (image: string, className: string) => {
    currentBubbleImageUrl.value = image;
    currentBubbleClass.value = className;
    if (!isBubbleVisible.value) {
      isBubbleVisible.value = true;
    }
    armBubbleHide();
  };

  // 头像文件随角色、情绪、衣服任一变化重新解析
  watch(
    () => [
      role.value.roleId,
      role.value.emotion,
      role.value.clothesName,
      role.value.character_folder,
    ],
    () => resolveAvatar(),
    { immediate: true },
  );

  // 监听表情，配合展示层的加载状态播放特效
  watch(
    () => role.value.emotion,
    async (newEmotion) => {
      const currentId = ++latestEmotionId;

      // 依次等待：头像路径解析 → DOM 更新传给子组件 → 子组件图片加载完成
      await resolveAvatar();
      await nextTick();
      await options.waitForAvatarLoad?.();

      // 陈旧性检查必须在注入的 await 之后：期间表情又变了就丢弃这次结果
      if (currentId !== latestEmotionId) return;

      const config = EMOTION_CONFIG[newEmotion];
      if (!config) return;

      if (config.animation && config.animation !== "none") {
        activeAnimationClass.value = config.animation;
      }

      if (config.bubbleImage && config.bubbleImage !== "none") {
        showEmotionBubble(config.bubbleImage, config.bubbleClass);
      }

      if (config.audio && config.audio !== "none") {
        playBubbleAudio(config.audio);
      }
    },
    { immediate: true },
  );

  // 气泡音量设置变化时，对已加载的音效实时生效
  watch(
    () => uiStore.bubbleVolume,
    (v) => {
      if (audioRef.value) audioRef.value.volume = v / 100;
    },
  );

  // 思考中反馈：气泡 + 音效（由 currentStatus 驱动，与 emotion 解耦）
  watch(
    () => gameStore.currentStatus,
    (newStatus) => {
      if (newStatus === "thinking") {
        const config = EMOTION_CONFIG["AI思考"];
        if (config && config.bubbleImage && config.bubbleImage !== "none") {
          showThinkingBubble(config.bubbleImage, config.bubbleClass);
        }
        if (config?.audio && config.audio !== "none") {
          playBubbleAudio(config.audio);
        }
      } else {
        // 离开思考态：隐藏思考气泡、停掉定时器
        isBubbleVisible.value = false;
        clearBubbleTimer();
      }
    },
  );

  const handleAnimationEnd = () => {
    if (activeAnimationClass.value !== "normal") {
      activeAnimationClass.value = "normal";
    }
  };

  // 卸载时清掉待触发的气泡定时器，避免组件已销毁还在写 ref
  onUnmounted(clearBubbleTimer);

  return {
    targetAvatarUrl,
    activeAnimationClass,
    isBubbleVisible,
    currentBubbleImageUrl,
    currentBubbleClass,
    resolveAvatar,
    handleAnimationEnd,
  };
}
