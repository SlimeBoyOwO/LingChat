import { ref, watch, onBeforeUnmount, type Ref } from "vue";
import { useUIStore } from "@lingchat/host/stores/modules/ui/ui";
import { getVoiceAudio } from "@lingchat/host/api/services/game-info";
import { setVoicePlaying } from "@lingchat/host/composables/asr";
import { isPluginVoicePlaybackActive } from "@lingchat/host/api/services/plugins";
export function useStoryVoice(
  mainAudio: Ref<HTMLAudioElement | null>,
  emit: (event: "audio-started" | "audio-ended") => void,
  options: { respectPluginVoicePlayback?: boolean } = {},
) {
  const uiStore = useUIStore();
  const voiceDataUrl = ref("");
  let voicePlaybackSeq = 0;
  const mutedForPlugin = ref(false);
  let pending = false;

  // 中断、加载失败与自然结束共用一个终态，避免自动推进一直等待或重复推进。
  const finishPlayback = () => {
    setVoicePlaying(false);
    if (!pending) return;
    pending = false;
    emit("audio-ended");
  };

  type PitchControllableAudio = HTMLAudioElement & {
    mozPreservesPitch?: boolean;
    webkitPreservesPitch?: boolean;
  };

  /** 恐怖剧本变速必须同时改变音高；显式关闭各 WebView 的“保持音高”。 */
  const applyVoiceRate = (audio: HTMLAudioElement, rate: number) => {
    const controllable = audio as PitchControllableAudio;
    controllable.preservesPitch = false;
    controllable.mozPreservesPitch = false;
    controllable.webkitPreservesPitch = false;
    controllable.playbackRate = rate > 0 ? rate : 1;
  };

  // ---------- Web Audio 纯降调路径（voice_shift 的 pitch != 0 时启用） ----------
  // AudioBufferSourceNode.detune 以音分（1/100 半音）为单位做纯音调偏移，不改变语速；
  // 与 playbackRate（变速变调）可叠加。HTMLAudioElement 无此能力，故 pitch 模式走这里。
  let audioCtx: AudioContext | null = null;
  let bufferSource: AudioBufferSourceNode | null = null;
  let gainNode: GainNode | null = null;
  const MAX_AUDIO_BUFFER_CACHE_BYTES = 64 * 1024 * 1024;
  const audioBufferCache = new Map<string, { buffer: AudioBuffer; bytes: number }>();
  const audioBufferLoads = new Map<string, Promise<AudioBuffer>>();
  let audioBufferCacheBytes = 0;
  let audioCacheGeneration = 0;

  const getAudioCtx = (): AudioContext => {
    if (!audioCtx) audioCtx = new AudioContext();
    return audioCtx;
  };

  const loadVoiceBuffer = async (dataUrl: string): Promise<AudioBuffer> => {
    const cached = audioBufferCache.get(dataUrl);
    if (cached) {
      // Map 插入顺序即 LRU 顺序；命中后移到末尾。
      audioBufferCache.delete(dataUrl);
      audioBufferCache.set(dataUrl, cached);
      return cached.buffer;
    }
    const loading = audioBufferLoads.get(dataUrl);
    if (loading) return loading;

    const generation = audioCacheGeneration;
    const promise = (async () => {
      const resp = await fetch(dataUrl);
      const buffer = await getAudioCtx().decodeAudioData(await resp.arrayBuffer());
      const bytes = buffer.length * buffer.numberOfChannels * Float32Array.BYTES_PER_ELEMENT;
      if (generation === audioCacheGeneration && bytes <= MAX_AUDIO_BUFFER_CACHE_BYTES) {
        while (audioBufferCacheBytes + bytes > MAX_AUDIO_BUFFER_CACHE_BYTES) {
          const oldestKey = audioBufferCache.keys().next().value as string | undefined;
          if (!oldestKey) break;
          const oldest = audioBufferCache.get(oldestKey);
          audioBufferCache.delete(oldestKey);
          audioBufferCacheBytes -= oldest?.bytes ?? 0;
        }
        audioBufferCache.set(dataUrl, { buffer, bytes });
        audioBufferCacheBytes += bytes;
      }
      return buffer;
    })();
    audioBufferLoads.set(dataUrl, promise);
    try {
      return await promise;
    } finally {
      if (audioBufferLoads.get(dataUrl) === promise) audioBufferLoads.delete(dataUrl);
    }
  };

  const stopWebAudio = () => {
    if (bufferSource) {
      bufferSource.onended = null;
      try {
        bufferSource.stop();
      } catch {
        // 未 start 或已停止：忽略
      }
      bufferSource.disconnect();
      bufferSource = null;
    }
    gainNode = null;
  };

  const playVoiceWithPitch = async (dataUrl: string, volume: number, playbackSeq: number) => {
    stopWebAudio();
    const buf = await loadVoiceBuffer(dataUrl);
    if (playbackSeq !== voicePlaybackSeq) return;
    const ctx = getAudioCtx();
    if (ctx.state === "suspended") await ctx.resume().catch(() => {});
    if (playbackSeq !== voicePlaybackSeq) return;
    const src = ctx.createBufferSource();
    src.buffer = buf;
    // 半音 → 音分
    src.detune.value = uiStore.voicePitch * 100;
    src.playbackRate.value = uiStore.voiceRate > 0 ? uiStore.voiceRate : 1;
    const gain = ctx.createGain();
    gain.gain.value = volume;
    src.connect(gain).connect(ctx.destination);
    src.onended = () => {
      if (playbackSeq !== voicePlaybackSeq) return;
      if (bufferSource === src) bufferSource = null;
      onAudioEnded();
    };
    bufferSource = src;
    gainNode = gain;
    setVoicePlaying(true);
    src.start();
    emit("audio-started");
  };

  // --- 音频逻辑 (全局) ---
  // 语音播放不走 useVoicePlayback 共用实现：voice_shift 的变速/纯降调需要 playbackRate +
  // Web Audio detune 双路径，composable 只包了 HTMLAudioElement 基础管线（桌宠仍用之）。
  // 监听 UI Store 的音频播放指令
  watch(
    () => uiStore.currentAvatarAudio,
    async (newAudio) => {
      if (!mainAudio.value) return;
      const playbackSeq = ++voicePlaybackSeq;

      // 如果设置为 'None'，停止当前播放
      if (newAudio === "None" || !newAudio) {
        voiceDataUrl.value = "";
        mainAudio.value.pause();
        mainAudio.value.currentTime = 0;
        stopWebAudio();
        mutedForPlugin.value = false;
        finishPlayback();
        return;
      }

      if (newAudio && newAudio !== "None") {
        // 与宿主基础管线一致：异步取音频期间也要阻止 ASR 录入自己的台词。
        pending = true;
        setVoicePlaying(true);
        try {
          const [dataUrl, takenOver] = await Promise.all([
            getVoiceAudio(newAudio),
            options.respectPluginVoicePlayback !== false
              ? isPluginVoicePlaybackActive().catch(() => false)
              : Promise.resolve(false),
          ]);
          if (playbackSeq !== voicePlaybackSeq) return;
          mutedForPlugin.value = takenOver;
          // 最新 Live2D 口型同步消费 voiceDataUrl；DLC 的 voice_shift 仍按需走 Web Audio。
          voiceDataUrl.value = dataUrl;
          const volume = takenOver ? 0 : uiStore.characterVolume / 100;
          if (uiStore.voicePitch !== 0) {
            // 纯降调模式：走 Web Audio detune（不变速），与 HTMLAudio 路径互斥
            mainAudio.value.pause();
            mainAudio.value.currentTime = 0;
            await playVoiceWithPitch(dataUrl, volume, playbackSeq);
            return;
          }
          stopWebAudio();
          mainAudio.value.src = dataUrl;
          mainAudio.value.load();
          mainAudio.value.volume = volume;
          // 剧本 voice_shift 恶魔音：降低播放倍率并关闭保音高。
          applyVoiceRate(mainAudio.value, uiStore.voiceRate);
          // TTS 播放中 ASR 禁用（外放 TTS 进麦克风会误识别 AI 自己的话）。
          mainAudio.value
            .play()
            .then(() => {
              if (playbackSeq !== voicePlaybackSeq) return;
              setVoicePlaying(true);
              emit("audio-started");
            })
            .catch((e) => {
              if (playbackSeq !== voicePlaybackSeq) return;
              console.error("播放失败", e);
              finishPlayback();
            });
        } catch (e) {
          console.error("获取语音文件失败:", e);
          if (playbackSeq === voicePlaybackSeq) finishPlayback();
        }
      }
    },
  );

  watch(
    () => uiStore.characterVolume,
    (v) => {
      const volume = mutedForPlugin.value ? 0 : v / 100;
      if (mainAudio.value) mainAudio.value.volume = volume;
      if (gainNode) gainNode.gain.value = volume;
    },
  );

  // voice_shift 发生在当前一句播放途中时也立即生效；后续新语音会在加载时再次应用。
  watch(
    () => uiStore.voiceRate,
    (rate) => {
      if (mainAudio.value) applyVoiceRate(mainAudio.value, rate);
      if (bufferSource) bufferSource.playbackRate.value = rate > 0 ? rate : 1;
    },
  );

  // 音调偏移（半音 → 音分）在播放途中同样即时生效
  watch(
    () => uiStore.voicePitch,
    (pitch) => {
      if (bufferSource) bufferSource.detune.value = pitch * 100;
    },
  );

  const onAudioEnded = () => {
    finishPlayback();
  };

  // 暴露停止音频的方法给父组件
  const stopAudio = () => {
    voicePlaybackSeq += 1;
    if (mainAudio.value) {
      mainAudio.value.pause();
      mainAudio.value.currentTime = 0;
    }
    stopWebAudio();
    finishPlayback();
  };

  onBeforeUnmount(() => {
    voicePlaybackSeq += 1;
    stopWebAudio();
    finishPlayback();
    audioCacheGeneration += 1;
    audioBufferLoads.clear();
    audioBufferCache.clear();
    audioBufferCacheBytes = 0;
    const ctx = audioCtx;
    audioCtx = null;
    if (ctx && ctx.state !== "closed") void ctx.close().catch(() => {});
  });

  return { voiceDataUrl, onAudioEnded, stopAudio };
}
