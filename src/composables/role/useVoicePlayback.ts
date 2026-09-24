/**
 * 角色语音（TTS）播放管线 —— 主界面（game/standard）与桌宠（pet）共用。
 *
 * 此前两个 GameRolesStage.vue 各有一份近乎逐字的实现。差异只有一处冗余判断
 * （主界面在已提前 return 的分支之后又判断了一次 `newAudio !== "None"`），
 * 已按统一后的形式收敛。
 *
 * 关键契约：TTS 播放期间必须 setVoicePlaying(true)。外放的 AI 语音会被麦克风
 * 收进去，不置位的话 ASR 会把 AI 自己的话当成用户输入。
 */
import { onUnmounted, ref, watch, type Ref } from "vue";
import { useUIStore } from "@/stores/modules/ui/ui";
import { getVoiceAudio } from "@/api/services/game-info";
import { setVoicePlaying } from "@/composables/asr";

export interface UseVoicePlaybackOptions {
  /** 模板里的 `<audio ref>` */
  audioRef: Ref<HTMLAudioElement | null>;
  /** play() 成功后触发 —— 组件在此 emit("audio-started") */
  onStarted?: () => void;
  /** `<audio>` 的 ended 事件触发 —— 组件在此 emit("audio-ended") */
  onEnded?: () => void;
}

export interface UseVoicePlaybackApi {
  /** 当前语音的 data URL，供 Live2DStage 做口型同步 */
  voiceDataUrl: Ref<string>;
  /** 绑定到 `<audio>` 的 @ended */
  onAudioEnded: () => void;
}

export function useVoicePlayback(options: UseVoicePlaybackOptions): UseVoicePlaybackApi {
  const { audioRef, onStarted, onEnded } = options;
  const uiStore = useUIStore();

  const voiceDataUrl = ref("");

  /** 停止播放并复位，同时解除 ASR 禁用 */
  const stopAudio = () => {
    if (!audioRef.value) return;
    audioRef.value.pause();
    audioRef.value.currentTime = 0;
    setVoicePlaying(false);
  };

  // 监听 UI Store 的音频播放指令
  watch(
    () => uiStore.currentAvatarAudio,
    async (newAudio) => {
      if (!audioRef.value) return;

      // 'None' 表示停止当前播放
      if (newAudio === "None" || !newAudio) {
        voiceDataUrl.value = "";
        stopAudio();
        return;
      }

      // 前置播放锁（审查 M4）：watch 触发即占位 voicePlaying——getVoiceAudio
      // 网络等待（100-500ms）与 play() 微任务延迟期间 ASR 不得触发录音
      //（TTS 已传出但 voicePlaying 未置位 → 会录进 AI 自己的话）
      setVoicePlaying(true);
      try {
        const dataUrl = await getVoiceAudio(newAudio);
        voiceDataUrl.value = dataUrl;
        audioRef.value.src = dataUrl;
        audioRef.value.load();
        audioRef.value.volume = uiStore.characterVolume / 100;
        // TTS 播放中 ASR 禁用（外放 TTS 进麦克风会误识别 AI 自己的话）
        audioRef.value
          .play()
          .then(() => {
            onStarted?.();
          })
          .catch((e) => {
            console.error("播放失败", e);
            setVoicePlaying(false);
          });
      } catch (e) {
        console.error("获取语音文件失败:", e);
        // 获取失败：播放不会发生 → 解除前置锁，否则 ASR 门控永久卡死
        setVoicePlaying(false);
      }
    },
  );

  // 音量设置变化时，对正在播放的语音实时生效
  watch(
    () => uiStore.characterVolume,
    (v) => {
      if (audioRef.value) audioRef.value.volume = v / 100;
    },
  );

  const onAudioEnded = () => {
    setVoicePlaying(false);
    onEnded?.();
  };

  // 路由切换（/chat ↔ /pet）销毁 audio 元素 → 播放被浏览器终止，ended 不触发：
  // 必须主动复位 voicePlaying，否则 ASR 第 12 项门控（TTS 播放中禁用）永久卡死，
  // PTT/mic/auto 全部静默失效直到下一次 TTS 自然播完
  onUnmounted(() => setVoicePlaying(false));

  return { voiceDataUrl, onAudioEnded };
}
