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
  /**
   * 本条语音「确定不会再播了」时触发一次 —— 组件在此 emit("audio-ended")。
   * 不只是 ended：被换源打断、收到 "None"、获取失败、play() 被拒、卸载都补报，
   * 否则下游（自动推进调度器）会永久卡住。
   */
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

  /**
   * 是否处于「已经开始播、还没结束」。ended 只在自然播完时触发（pause、换 src、
   * load 都不触发），所以中断要靠这个标志补报一次结束。
   */
  let playing = false;

  /** 单一出口：只在确实在播时报一次，避免 ended/error 双触发重复上报 */
  const emitEndedOnce = () => {
    if (!playing) return;
    playing = false;
    onEnded?.();
  };

  /** 停止播放并复位，同时解除 ASR 禁用 */
  const stopAudio = () => {
    // 元素可能还没挂上，但「结束」仍要上报，不能在这里早退
    if (audioRef.value) {
      audioRef.value.pause();
      audioRef.value.currentTime = 0;
    }
    setVoicePlaying(false);
    emitEndedOnce();
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
      // 换源会打断上一条但它是终态；这里只清标志不补报，新的一条马上就要 started
      playing = false;
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
            // 与 onStarted 同时置位：提前到 watch 入口会把网络等待期的前置锁误当成在播
            playing = true;
            onStarted?.();
          })
          .catch((e) => {
            console.error("播放失败", e);
            setVoicePlaying(false);
            emitEndedOnce(); // 终态：本条不会再播完，必须放行下游
          });
      } catch (e) {
        console.error("获取语音文件失败:", e);
        // 获取失败：播放不会发生 → 解除前置锁，否则 ASR 门控永久卡死
        setVoicePlaying(false);
        emitEndedOnce();
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

  // 模板的 @ended / @error 都走同一个出口，否则两者先后触发会报两次
  const onAudioEnded = () => {
    setVoicePlaying(false);
    emitEndedOnce();
  };

  // 路由切换（/chat ↔ /pet）销毁 audio 元素 → 播放被浏览器终止，ended 不触发：
  // 必须主动复位 voicePlaying，否则 ASR 第 12 项门控（TTS 播放中禁用）永久卡死，
  // PTT/mic/auto 全部静默失效直到下一次 TTS 自然播完。结束回报同理要补。
  onUnmounted(() => {
    setVoicePlaying(false);
    emitEndedOnce();
  });

  return { voiceDataUrl, onAudioEnded };
}
