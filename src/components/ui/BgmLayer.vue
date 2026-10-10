<template>
  <!-- 全局唯一背景音乐层：由 App.vue 在主窗口常驻挂载一次，
       <audio> 不随路由卸载，故 /chat 与 /pet 之间切换时音乐不中断、不从头重播。
       保留真实 DOM <audio>（AudioAcrossFade 内含两条、带 crossorigin），
       以继续兼容 audioOutputManager 的 DOM 扫描与频谱采集。 -->
  <AudioAcrossFade
    :src="backgroundMusicSrc"
    :volume="uiStore.backgroundVolume"
    :paused="bgmPaused"
    :stopped="uiStore.bgMusicStoped"
    :rate="uiStore.bgMusicPlaybackRate"
    :duration="800"
    :loop="uiStore.bgMusicMode === 'loop-single'"
    @ended="handleTrackEnd"
  />
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { useUIStore } from "@/stores/modules/ui/ui";
import { musicGetAll } from "@/api/services/music";
import { toPlayableMediaUrl } from "@/utils/mediaUrl";
import AudioAcrossFade from "./AudioAcrossFade.vue";
import type { MusicTrack } from "@/types";

const uiStore = useUIStore();
const route = useRoute();

// 允许出声：聊天 / 桌宠路由，或当前有游戏渲染层在场 ——
// 后者覆盖剧本编辑器试玩覆盖层，也能自动适配未来复用 GameBackground 的场景。
// 主菜单、工坊、非试玩的编辑器一律静音。
const routeAllowsBgm = computed(
  () => route.path === "/chat" || route.path === "/pet" || uiStore.gameLayerCount > 0,
);

// 手动暂停 或 当前路由不该出声 → 暂停。
// 只暂停不重置，<audio> 与 currentTime 都保留，回到允许场景时接着原进度播
const bgmPaused = computed(() => uiStore.bgMusicPaused || !routeAllowsBgm.value);

// 原始路径 → 可播放 URL（Android 走 blob），由本层统一做转换。
// immediate：本层在启动时即挂载，currentBackgroundMusic 可能已被存档恢复
const backgroundMusicSrc = ref("None");
let bgmLoadSeq = 0;
watch(
  () => uiStore.currentBackgroundMusic,
  async (src) => {
    const seq = ++bgmLoadSeq;
    if (!src || src === "None") {
      backgroundMusicSrc.value = "None";
      return;
    }
    try {
      const url = await toPlayableMediaUrl(src);
      // 竞态守卫：期间已切换过歌曲则丢弃过期结果
      if (seq !== bgmLoadSeq) return;
      backgroundMusicSrc.value = url;
    } catch (e) {
      if (seq !== bgmLoadSeq) return;
      console.warn("背景音乐加载失败:", src, e);
      backgroundMusicSrc.value = "None";
    }
  },
  { immediate: true },
);

// 当前曲目自然播完（单曲循环由 AudioAcrossFade 内部处理，不会走到这里）→ 按播放模式接下一首。
// 原先这段逻辑在声音设置页里，只有设置页打开时才生效
const handleTrackEnd = async () => {
  if (uiStore.bgMusicMode === "loop-single") return;

  let list: MusicTrack[];
  try {
    list = await musicGetAll();
  } catch (e) {
    console.warn("加载音乐列表失败:", e);
    return;
  }
  if (list.length === 0) return;

  const currentIndex = list.findIndex((m) => m.url === uiStore.currentBackgroundMusic);
  const next =
    uiStore.bgMusicMode === "random"
      ? list[Math.floor(Math.random() * list.length)]
      : list[currentIndex === -1 ? 0 : (currentIndex + 1) % list.length];
  if (!next) return;

  uiStore.currentBackgroundMusic = next.url;
  uiStore.bgMusicPaused = false;
  uiStore.bgMusicStoped = false;
};
</script>
