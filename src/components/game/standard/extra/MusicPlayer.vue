<template>
  <transition @before-enter="beforeEnter" @enter="enter" @before-leave="beforeLeave" @leave="leave">
    <div
      v-if="isVisible"
      class="pointer-events-auto fixed bottom-[calc(24px+var(--safe-area-inset-bottom))] left-0
        z-1000 drop-shadow-[0_8px_12px_rgba(0,0,0,0.25)]"
    >
      <div
        class="relative flex items-center gap-1 overflow-hidden rounded-tr-xl bg-slate-900/40 py-2
          pr-5 pl-4 backdrop-blur-md"
      >
        <!-- 左侧装饰条 -->
        <div
          class="absolute top-0 bottom-0 left-0 w-1 rounded-r-sm"
          style="background: var(--accent-color)"
        ></div>

        <!-- 音符图标 + 歌名 -->
        <div class="flex min-w-0 items-center gap-2">
          <Music
            :size="16"
            class="shrink-0"
            style="color: var(--accent-color)"
            :class="{ 'animate-pulse': !isPaused }"
          />
          <span
            class="max-w-40 truncate text-sm font-medium text-gray-200"
            :title="currentMusicName"
          >
            {{ currentMusicName }}
          </span>
        </div>

        <!-- 分隔线 -->
        <div class="h-5 w-px shrink-0 bg-white/15"></div>

        <!-- 控制按钮 -->
        <div class="flex items-center gap-0.5">
          <button
            class="rounded-lg p-1.5 text-gray-400 transition-colors duration-200 hover:bg-white/10
              hover:text-white"
            @click="handlePrevious"
            :title="$t('game.musicPlayer.previous')"
          >
            <SkipBack :size="16" />
          </button>
          <button
            class="rounded-lg p-1.5 text-gray-400 transition-colors duration-200 hover:bg-white/10
              hover:text-white"
            @click="handlePlayPause"
            :title="isPaused ? $t('game.musicPlayer.play') : $t('game.musicPlayer.pause')"
          >
            <Play v-if="isPaused" :size="16" />
            <Pause v-else :size="16" />
          </button>
          <button
            class="rounded-lg p-1.5 text-gray-400 transition-colors duration-200 hover:bg-white/10
              hover:text-white"
            @click="handleNext"
            :title="$t('game.musicPlayer.next')"
          >
            <SkipForward :size="16" />
          </button>
        </div>
      </div>
    </div>
  </transition>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { Music, Play, Pause, SkipBack, SkipForward } from "lucide-vue-next";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { musicGetAll } from "@/api/services/music";
  import type { MusicTrack } from "@/types";

  const uiStore = useUIStore();
  const { t } = useI18n();

  const musicList = ref<MusicTrack[]>([]);
  const currentMusicName = ref("");

  const isPaused = computed(() => uiStore.bgMusicPaused);
  const isStopped = computed(() => uiStore.bgMusicStoped);

  const isVisible = computed(() => {
    if (uiStore.showSettings) return false;
    if (isStopped.value) return false;
    return uiStore.currentBackgroundMusic !== "None" && musicList.value.length > 0;
  });

  const inferMusicNameFromUrl = (musicUrl: string): string => {
    if (!musicUrl || musicUrl === "None") return t("game.musicPlayer.noMusic");
    const fileName = decodeURIComponent(musicUrl.split("/").pop() || "");
    if (!fileName) return t("game.musicPlayer.noMusic");
    return fileName.replace(/\.[^/.]+$/, "") || fileName;
  };

  const syncCurrentMusicName = () => {
    const currentUrl = uiStore.currentBackgroundMusic;
    if (!currentUrl || currentUrl === "None") {
      currentMusicName.value = t("game.musicPlayer.noMusic");
      return;
    }
    const matched = musicList.value.find((item) => item.url === currentUrl);
    currentMusicName.value = matched?.name || inferMusicNameFromUrl(currentUrl);
  };

  const getCurrentIndex = (): number => {
    const currentUrl = uiStore.currentBackgroundMusic;
    if (!currentUrl) return -1;
    return musicList.value.findIndex((m) => m.url === currentUrl);
  };

  const handlePlayPause = () => {
    if (uiStore.currentBackgroundMusic === "None" && musicList.value.length > 0) {
      const music = musicList.value[0];
      if (music) {
        uiStore.currentBackgroundMusic = music.url;
        uiStore.bgMusicPaused = false;
        uiStore.bgMusicStoped = false;
      }
    } else {
      uiStore.bgMusicPaused = !uiStore.bgMusicPaused;
    }
  };

  const handlePrevious = () => {
    if (musicList.value.length === 0) return;
    const currentIndex = getCurrentIndex();
    const prevIndex = currentIndex <= 0 ? musicList.value.length - 1 : currentIndex - 1;
    const music = musicList.value[prevIndex];
    if (music) {
      uiStore.currentBackgroundMusic = music.url;
      uiStore.bgMusicPaused = false;
      uiStore.bgMusicStoped = false;
    }
  };

  const handleNext = () => {
    if (musicList.value.length === 0) return;
    const currentIndex = getCurrentIndex();
    const nextIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % musicList.value.length;
    const music = musicList.value[nextIndex];
    if (music) {
      uiStore.currentBackgroundMusic = music.url;
      uiStore.bgMusicPaused = false;
      uiStore.bgMusicStoped = false;
    }
  };

  // 监听音乐结束事件
  watch(
    () => uiStore._musicEndTime,
    () => {
      if (uiStore.bgMusicMode === "loop-single") return;
      handleNext();
    }
  );

  watch(
    () => uiStore.currentBackgroundMusic,
    () => syncCurrentMusicName()
  );

  const loadMusicList = async () => {
    try {
      musicList.value = await musicGetAll();
      syncCurrentMusicName();
    } catch (e) {
      console.error("MusicPlayer: 加载音乐列表失败", e);
    }
  };

  // 动画钩子
  function beforeEnter(el: Element) {
    const element = el as HTMLElement;
    element.style.opacity = "0";
    element.style.transform = "translateX(-20px)";
    element.style.transition = "all 0.4s cubic-bezier(0.4, 0, 0.2, 1)";
  }

  function enter(el: Element, done: () => void) {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        const element = el as HTMLElement;
        element.style.opacity = "1";
        element.style.transform = "translateX(0)";
        setTimeout(done, 400);
      });
    });
  }

  function beforeLeave(el: Element) {
    const element = el as HTMLElement;
    element.style.opacity = "1";
    element.style.transform = "translateX(0)";
    element.style.transition = "all 0.3s cubic-bezier(0.4, 0, 0.2, 1)";
  }

  function leave(el: Element, done: () => void) {
    const element = el as HTMLElement;
    element.style.opacity = "0";
    element.style.transform = "translateX(-20px)";
    setTimeout(done, 300);
  }

  onMounted(() => {
    loadMusicList();
  });
</script>
