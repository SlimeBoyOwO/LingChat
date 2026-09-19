<template>
  <MenuPage>
    <!-- 输出音频设备切换（顶部） -->
    <MenuItem :title="$t('settings.sound.device.title')">
      <template #header>
        <Speaker :size="20" class="text-amber-400" />
      </template>
      <div v-if="audioOutputSupported" class="flex w-full flex-col gap-2">
        <div class="flex w-full items-stretch gap-2">
          <select
            :value="currentDeviceId"
            class="output-device-select min-w-0 flex-1 cursor-pointer appearance-none rounded-lg
              border border-white/20 bg-white/10 py-2 pr-8 pl-3 text-sm text-white transition-colors
              outline-none focus:border-(--accent-color)"
            style="color-scheme: dark"
            @change="onOutputDeviceChange"
            :title="$t('settings.sound.device.selectTitle')"
          >
            <option value="">{{ $t("settings.sound.device.systemDefault") }}</option>
            <option v-for="d in outputDevices" :key="d.deviceId" :value="d.deviceId">
              {{ d.label }}{{ d.isDefault ? $t("settings.sound.device.currentDefault") : "" }}
            </option>
          </select>
          <button
            v-if="!labelsAvailable"
            type="button"
            class="inline-flex flex-none cursor-pointer items-center justify-center gap-1 rounded-lg
              border border-white/20 bg-white/10 px-3 py-2 text-sm text-white transition-colors
              hover:border-(--accent-color) hover:bg-white/20 active:bg-white/30"
            @click="requestDeviceLabelsAndRefresh"
            :title="
              permissionDenied
                ? $t('settings.sound.device.retryLabels')
                : $t('settings.sound.device.getLabels')
            "
          >
            <ScanLine :size="15" />
            {{
              permissionDenied
                ? $t("settings.sound.device.retryLabels")
                : $t("settings.sound.device.getLabels")
            }}
          </button>
        </div>

        <div class="w-full">
          <span class="block truncate text-xs text-gray-300">{{ currentDeviceLabel }}</span>
          <p
            v-if="!labelsAvailable && permissionDenied"
            class="mt-1 text-xs leading-snug text-amber-400/80"
          >
            {{ $t("settings.sound.device.labelsDenied") }}
          </p>
        </div>
      </div>
      <p v-else class="text-xs text-gray-400">{{ $t("settings.sound.device.unsupported") }}</p>
    </MenuItem>

    <!-- 音量控制部分 -->
    <MenuItem :title="$t('settings.sound.volume.character')" size="small">
      <template #header>
        <MicVocal :size="20" class="text-indigo-400" />
      </template>
      <Slider v-model="characterVolume" @change="updateCharacterVolume">
        {{ $t("settings.sound.slider.weakStrong") }}
      </Slider>
    </MenuItem>

    <MenuItem :title="$t('settings.sound.volume.bubble')" size="small">
      <template #header>
        <MessageCircle :size="20" class="text-blue-400" />
      </template>
      <Slider @change="updateBubbleVolume" v-model="bubbleVolume">
        {{ $t("settings.sound.slider.weakStrong") }}
      </Slider>
    </MenuItem>

    <MenuItem :title="$t('settings.sound.volume.background')" size="small">
      <template #header>
        <AudioLines :size="20" class="text-green-400" />
      </template>
      <Slider @change="updateBackgroundVolume" v-model="backgroundVolume">
        {{ $t("settings.sound.slider.weakStrong") }}
      </Slider>
    </MenuItem>

    <MenuItem :title="$t('settings.sound.volume.achievement')" size="small">
      <template #header>
        <Trophy :size="20" class="text-yellow-400" />
      </template>
      <Slider @change="updateAchievementVolume" v-model="achievementVolume">
        {{ $t("settings.sound.slider.weakStrong") }}
      </Slider>
    </MenuItem>

    <!-- 测试声音部分 -->
    <!-- 环境音音量控制 -->
    <MenuItem :title="$t('settings.sound.volume.ambient')" size="small">
      <template #header>
        <CloudRain :size="20" class="text-cyan-400" />
      </template>
      <Slider @change="updateAmbientVolume" v-model="ambientVolume">
        {{ $t("settings.sound.slider.weakStrong") }}
      </Slider>
    </MenuItem>

    <!-- 声音测试 -->
    <MenuItem :title="$t('settings.sound.test.title')" size="small">
      <template #header>
        <FlaskConical :size="20" class="text-pink-400" />
      </template>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <Button type="big" class="min-w-30 flex-1" @click="playCharacterTestSound">{{
          $t("settings.sound.test.character")
        }}</Button>
        <Button type="big" class="min-w-30 flex-1" @click="playBubbleTestSound">{{
          $t("settings.sound.test.bubble")
        }}</Button>
        <Button type="big" class="min-w-30 flex-1" @click="playAchievementTestSound">{{
          $t("settings.sound.test.achievement")
        }}</Button>
      </div>
    </MenuItem>

    <!-- 背景音乐设置部分 -->
    <MenuItem :title="$t('settings.sound.bgm.title')">
      <template #header>
        <Headphones :size="20" class="text-purple-400" />
      </template>

      <!-- 音乐控制台 -->
      <div
        class="flex gap-3 rounded-xl border border-white/10 bg-white/5 p-4 backdrop-blur-md
          max-[640px]:flex-col max-[640px]:gap-2"
      >
        <div
          class="flex w-[60%] items-center justify-between rounded-lg bg-black/20 px-3 py-2 text-sm
            font-medium text-gray-200 max-[640px]:w-full"
        >
          <span class="flex items-center gap-2 truncate">
            <Music :size="16" class="shrink-0 text-purple-400" />
            <span class="truncate">{{ currentMusicName }}</span>
          </span>
          <span class="ml-2 shrink-0 text-xs text-gray-400">{{
            modeText[uiStore.bgMusicMode]
          }}</span>
        </div>

        <div class="flex w-[40%] items-center gap-2 max-[640px]:w-full max-[640px]:flex-wrap">
          <Button
            type="big"
            @click="handlePlayPause"
            class="flex items-center justify-center gap-1"
          >
            <Play v-if="uiStore.bgMusicPaused" :size="16" />
            <Pause v-else :size="16" />
            {{ playPauseButtonText }}
          </Button>
          <Button type="big" @click="handleStop" class="flex items-center justify-center gap-1">
            <Square :size="14" /> {{ $t("settings.sound.bgm.stop") }}
          </Button>
          <Button
            type="big"
            @click="togglePlaybackMode"
            class="flex items-center justify-center"
            :title="modeText[uiStore.bgMusicMode]"
          >
            <Repeat1 v-if="uiStore.bgMusicMode === 'loop-single'" :size="18" />
            <Repeat v-else-if="uiStore.bgMusicMode === 'loop-list'" :size="18" />
            <Shuffle v-else :size="18" />
          </Button>
        </div>
      </div>

      <!-- 音乐分类管理（子文件夹 = 子分类）：选项卡 + 新建 + 删除 + 刷新 -->
      <div class="mt-4 mb-2 flex flex-wrap items-center gap-2">
        <button
          class="rounded-full border px-3 py-1 text-xs font-semibold transition-all"
          :class="
            currentMusicCategory === '全部'
              ? 'border-purple-400 bg-purple-500/80 text-white'
              : 'border-white/20 bg-white/10 text-white/70 hover:bg-white/20'
          "
          @click="currentMusicCategory = '全部'"
        >
          {{ $t("settings.sound.bgm.categoryAll") }}
        </button>
        <button
          v-for="cat in musicCategories"
          :key="cat"
          class="rounded-full border px-3 py-1 text-xs font-semibold transition-all"
          :class="
            currentMusicCategory === cat
              ? 'border-purple-400 bg-purple-500/80 text-white'
              : 'border-white/20 bg-white/10 text-white/70 hover:bg-white/20'
          "
          @click="currentMusicCategory = cat"
        >
          {{ cat }}
        </button>
        <!-- 新建分类 -->
        <div class="flex items-center gap-1">
          <input
            v-model="newMusicCategoryName"
            :placeholder="$t('settings.sound.bgm.categoryNamePlaceholder')"
            class="w-28 rounded-lg border border-white/15 bg-black/30 px-2 py-1 text-xs text-white
              focus:border-purple-400 focus:outline-none"
            @keyup.enter="handleCreateMusicCategory"
          />
          <button
            class="rounded-full border border-purple-400 bg-purple-500/80 px-2.5 py-1 text-xs
              font-semibold text-white hover:bg-purple-500"
            @click="handleCreateMusicCategory"
          >
            {{ $t("settings.sound.bgm.categoryAdd") }}
          </button>
        </div>
        <!-- 删除当前选中的分类 -->
        <button
          v-if="currentMusicCategory !== '全部' && !isMusicCategoryReadOnly"
          class="rounded-full border border-red-400/40 bg-red-500/20 px-2.5 py-1 text-xs
            font-semibold text-red-300 hover:bg-red-500/30"
          @click="handleDeleteMusicCategory"
        >
          {{ $t("settings.sound.bgm.categoryDelete") }}
        </button>
        <!-- 刷新 -->
        <button
          class="rounded-full border border-white/20 bg-white/10 px-2.5 py-1 text-xs font-semibold
            text-white/70 hover:bg-white/20"
          @click="handleRefreshMusic"
        >
          {{ $t("settings.sound.bgm.refresh") }}
        </button>
      </div>

      <!-- 音乐列表 -->
      <div
        class="mt-4 flex flex-col overflow-hidden rounded-xl border border-white/10 bg-black/20
          backdrop-blur-sm"
      >
        <div v-if="filteredMusicList.length === 0" class="py-8 text-center text-sm text-gray-400">
          {{ $t("settings.sound.bgm.empty") }}
        </div>
        <div v-else class="custom-scrollbar max-h-52 space-y-1 overflow-y-auto p-1.5">
          <div
            v-for="music in filteredMusicList"
            :key="music.url"
            @click="playMusic(music)"
            class="group flex cursor-pointer items-center justify-between rounded-lg px-3 py-2.5
              transition-all duration-200 hover:bg-white/10"
            :class="{ 'bg-purple-500/20 text-purple-300': currentMusicName === music.name }"
          >
            <div
              class="flex flex-1 items-center gap-2 overflow-hidden pr-2 text-sm font-medium
                text-ellipsis whitespace-nowrap"
            >
              <button
                @click.stop="handleToggleMusicFavorite(music.url)"
                class="shrink-0 rounded p-0.5 opacity-0 transition-opacity group-hover:opacity-100"
                :title="
                  isMusicFavored(music.url)
                    ? $t('settings.sound.bgm.unfav')
                    : $t('settings.sound.bgm.fav')
                "
              >
                <Star
                  :size="14"
                  :class="
                    isMusicFavored(music.url)
                      ? 'fill-amber-400 text-amber-400'
                      : 'text-white/60 hover:text-white'
                  "
                />
              </button>
              <span class="truncate">{{ music.name }}</span>
              <PluginTag v-if="music.source && music.source !== 'game'" :source="music.source" />
            </div>
            <button
              v-if="!music.source || music.source === 'game'"
              @click.stop="deleteMusic(music)"
              class="rounded-md bg-red-500/10 p-1.5 text-red-400 opacity-0 transition-opacity
                duration-200 group-hover:opacity-100 hover:bg-red-500/80 hover:text-white"
              :title="$t('settings.sound.common.delete')"
            >
              <Trash2 :size="14" />
            </button>
          </div>
        </div>
      </div>

      <!-- 批量上传区域 -->
      <div class="mt-4 flex items-center gap-3">
        <Button
          type="big"
          @click="triggerFileUpload"
          :disabled="isMusicCategoryReadOnly"
          class="flex flex-1 items-center justify-center gap-2"
          :class="{ 'cursor-not-allowed opacity-50': isMusicCategoryReadOnly }"
        >
          <UploadCloud :size="18" /> {{ $t("settings.sound.bgm.add") }}
        </Button>
        <Button
          type="big"
          @click="handleOpenMusicFolder"
          class="flex flex-1 items-center justify-center gap-2"
        >
          <FolderOpen :size="18" /> {{ $t("settings.sound.bgm.openFolder") }}
        </Button>
        <div class="flex flex-1 items-center justify-between gap-2">
          <span class="w-24 truncate text-xs text-gray-400" v-if="selectedPaths.length > 0">
            {{ $t("settings.sound.common.selectedCount", { count: selectedPaths.length }) }}
          </span>
          <span class="w-24 truncate text-xs text-gray-500" v-else>{{
            $t("settings.sound.common.noSelection")
          }}</span>

          <Button
            type="big"
            @click="uploadMusic"
            :disabled="selectedPaths.length === 0 || isMusicCategoryReadOnly"
            class="flex-1"
            :class="{
              'cursor-not-allowed opacity-50':
                selectedPaths.length === 0 || isMusicCategoryReadOnly,
            }"
          >
            {{ $t("settings.sound.common.confirmUpload") }}
          </Button>
        </div>
      </div>
    </MenuItem>

    <!-- 环境音管理 -->
    <MenuItem :title="$t('settings.sound.ambient.title')">
      <template #header>
        <Wind :size="20" class="text-teal-400" />
      </template>

      <!-- 环境音文件库 -->
      <div class="mb-2 flex items-center justify-between">
        <span class="text-xs text-gray-400">{{ $t("settings.sound.ambient.importedFiles") }}</span>
        <span class="text-xs text-gray-500">{{
          $t("settings.sound.ambient.fileCount", { count: ambientFileList.length })
        }}</span>
      </div>
      <div
        class="flex flex-col overflow-hidden rounded-xl border border-white/10 bg-black/20
          backdrop-blur-sm"
      >
        <div v-if="ambientFileList.length === 0" class="py-4 text-center text-sm text-gray-400">
          {{ $t("settings.sound.ambient.empty") }}
        </div>
        <div v-else class="custom-scrollbar max-h-32 space-y-1 overflow-y-auto p-1.5">
          <div
            v-for="ambient in ambientFileList"
            :key="ambient.url"
            class="group flex items-center gap-2 rounded-lg bg-white/5 px-3 py-2 transition-all
              duration-200 hover:bg-white/10"
          >
            <Wind :size="13" class="shrink-0 text-teal-400/60" />
            <span class="flex-1 truncate text-sm text-gray-200">{{ ambient.name }}</span>
            <PluginTag
              v-if="ambient.source && ambient.source !== 'game'"
              :source="ambient.source"
            />
            <button
              @click="addFileToTrack(ambient)"
              class="rounded bg-teal-500/20 px-2 py-0.5 text-xs text-teal-300 opacity-70
                transition-opacity hover:bg-teal-500/40 hover:opacity-100"
              :title="$t('settings.sound.ambient.addToTrack')"
            >
              {{ $t("settings.sound.ambient.play") }}
            </button>
            <button
              v-if="!ambient.source || ambient.source === 'game'"
              @click.stop="removeAmbientFile(ambient)"
              class="rounded-md bg-red-500/10 p-1.5 text-red-400 opacity-0 transition-opacity
                group-hover:opacity-100 hover:bg-red-500/80 hover:text-white"
              :title="$t('settings.sound.common.delete')"
            >
              <Trash2 :size="14" />
            </button>
          </div>
        </div>
      </div>

      <!-- 上传环境音 -->
      <div class="mt-2 flex items-center gap-3">
        <Button
          type="big"
          @click="triggerAmbientUpload"
          class="flex flex-1 items-center justify-center gap-2"
        >
          <UploadCloud :size="16" /> {{ $t("settings.sound.ambient.add") }}
        </Button>
        <div class="flex flex-1 items-center justify-between gap-2">
          <span class="w-24 truncate text-xs text-gray-400" v-if="selectedAmbientPaths.length > 0">
            {{ $t("settings.sound.common.selectedCount", { count: selectedAmbientPaths.length }) }}
          </span>
          <span class="w-24 truncate text-xs text-gray-500" v-else>{{
            $t("settings.sound.common.noSelection")
          }}</span>
          <Button
            type="big"
            @click="uploadAmbientFiles"
            :disabled="selectedAmbientPaths.length === 0"
            class="flex-1"
            :class="{ 'cursor-not-allowed opacity-50': selectedAmbientPaths.length === 0 }"
          >
            {{ $t("settings.sound.common.confirmUpload") }}
          </Button>
        </div>
      </div>

      <!-- 活跃轨道（带单轨音量控制） -->
      <div class="mt-4 mb-2 flex items-center justify-between">
        <span class="text-xs text-gray-400">{{ $t("settings.sound.ambient.playing") }}</span>
        <span class="text-xs text-gray-500"
          >{{ uiStore.ambientTracks.length }}/{{ maxAmbientTracks }}</span
        >
      </div>
      <div
        class="flex flex-col overflow-hidden rounded-xl border border-white/10 bg-black/20
          backdrop-blur-sm"
      >
        <div
          v-if="uiStore.ambientTracks.length === 0"
          class="py-4 text-center text-sm text-gray-400"
        >
          {{ $t("settings.sound.ambient.noPlaying") }}
        </div>
        <div v-else class="custom-scrollbar max-h-48 space-y-1 overflow-y-auto p-1.5">
          <div
            v-for="track in uiStore.ambientTracks"
            :key="track.id"
            class="group flex flex-col gap-1.5 rounded-lg bg-cyan-500/10 px-3 py-2 transition-all
              duration-200"
          >
            <!-- 轨道名 + 控制按钮 -->
            <div class="flex items-center gap-2">
              <CloudRain :size="14" class="shrink-0 text-cyan-400" />
              <span class="flex-1 truncate text-sm text-gray-200">{{
                getTrackDisplayName(track)
              }}</span>
              <button
                @click="uiStore.toggleAmbientTrackPause(track.id)"
                class="rounded p-1 text-gray-400 transition-colors hover:bg-white/10
                  hover:text-white"
                :title="
                  track.paused
                    ? $t('settings.sound.ambient.resume')
                    : $t('settings.sound.ambient.pause')
                "
              >
                <Play v-if="track.paused" :size="12" />
                <Pause v-else :size="12" />
              </button>
              <button
                @click="uiStore.removeAmbientTrack(track.id)"
                class="rounded bg-red-500/10 p-1 text-red-400 opacity-0 transition-opacity
                  group-hover:opacity-100 hover:bg-red-500/40 hover:text-white"
                :title="$t('settings.sound.ambient.removeTrack')"
              >
                <X :size="12" />
              </button>
            </div>
            <!-- 单轨音量滑块 -->
            <div class="flex items-center gap-2 pl-6">
              <span class="w-8 shrink-0 text-xs text-gray-400">{{
                $t("settings.sound.ambient.volumeLabel")
              }}</span>
              <Slider
                :model-value="track.volume"
                @change="(val: number) => uiStore.updateAmbientTrackVolume(track.id, val)"
                class="flex-1"
              />
              <span class="w-8 shrink-0 text-right text-xs text-gray-400">{{ track.volume }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 全部停止 -->
      <div class="mt-2">
        <Button
          type="big"
          @click="stopAllAmbient"
          class="flex w-full items-center justify-center gap-1"
          :disabled="uiStore.ambientTracks.length === 0"
        >
          <Square :size="14" /> {{ $t("settings.sound.ambient.stopAll") }}
        </Button>
      </div>
    </MenuItem>

    <!-- 音频播放器 (隐藏) -->
    <audio ref="characterTestPlayer"></audio>
    <audio ref="bubbleTestPlayer"></audio>
    <audio ref="achievementTestPlayer"></audio>
  </MenuPage>
</template>

<script setup lang="ts">
  import { computed, onMounted, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { Button, Slider } from "../../base";
  import { MenuItem, MenuPage } from "../../ui";
  import PluginTag from "@/components/ui/PluginTag.vue";
  import { musicDialogFilters } from "@/utils/dialogFilters";
  import {
    musicDelete,
    musicGetAll,
    musicUpload,
    musicListCategories,
    musicCreateCategory,
    musicDeleteCategory,
    openMusicFolder,
    setCurrentBackgroundMusic,
  } from "../../../api/services/music";
  import {
    ambientGetAll,
    ambientUpload,
    ambientDelete,
    type AmbientItem,
  } from "../../../api/services/ambient";
  import { useUIStore } from "../../../stores/modules/ui/ui";
  import { useDialogStore } from "../../../stores/modules/ui/dialog";
  import { useRoleArchiveStore } from "../../../stores/modules/ui/role-archive";
  import { useSettingsStore } from "../../../stores/modules/settings";
  import {
    currentDeviceId,
    devices as outputDevices,
    labelsAvailable,
    permissionDenied,
    refreshDevices,
    requestDeviceLabels,
    setDevice,
    supported as audioOutputSupported,
  } from "../../../utils/audioOutputManager";
  import { decodePathFileName } from "../../../utils/path";
  import {
    AudioLines,
    FlaskConical,
    MessageCircle,
    MicVocal,
    Trophy,
    Headphones,
    Play,
    Pause,
    Square,
    Repeat,
    Repeat1,
    Shuffle,
    Trash2,
    UploadCloud,
    Music,
    CloudRain,
    Wind,
    X,
    Speaker,
    ScanLine,
    Star,
    FolderOpen,
  } from "lucide-vue-next";

  const uiStore = useUIStore();
  const settingsStore = useSettingsStore();
  const dialogStore = useDialogStore();
  const roleStore = useRoleArchiveStore();
  const { t } = useI18n();

  // 状态绑定
  const characterVolume = computed({
    get: () => settingsStore.characterVolume,
    set: (val: number) => settingsStore.update("audio.characterVolume", val),
  });
  const bubbleVolume = computed({
    get: () => settingsStore.bubbleVolume,
    set: (val: number) => settingsStore.update("audio.bubbleVolume", val),
  });
  const backgroundVolume = computed({
    get: () => settingsStore.backgroundVolume,
    set: (val: number) => settingsStore.update("audio.backgroundVolume", val),
  });
  const achievementVolume = computed({
    get: () => settingsStore.achievementVolume,
    set: (val: number) => settingsStore.update("audio.achievementVolume", val),
  });
  // 环境音音量双向绑定
  const ambientVolume = computed({
    get: () => settingsStore.ambientVolume,
    set: (val: number) => settingsStore.update("audio.ambientVolume", val),
  });

  // ========== 输出音频设备（设置顶部） ==========
  const currentDeviceLabel = computed(() => {
    if (!currentDeviceId.value) return t("settings.sound.device.usingSystemDefault");
    const found = outputDevices.value.find((d) => d.deviceId === currentDeviceId.value);
    return found
      ? t("settings.sound.device.currentOutput", { label: found.label })
      : t("settings.sound.device.unavailableFallback");
  });

  const onOutputDeviceChange = async (e: Event) => {
    const target = e.target as HTMLSelectElement;
    await setDevice(target.value);
  };

  const requestDeviceLabelsAndRefresh = async () => {
    await requestDeviceLabels(true);
    await refreshDevices(false);
  };

  // 音频引用
  const characterTestPlayer = ref<HTMLAudioElement | null>(null);
  const bubbleTestPlayer = ref<HTMLAudioElement | null>(null);
  const achievementTestPlayer = ref<HTMLAudioElement | null>(null);
  const backgroundAudioPlayer = ref<HTMLAudioElement | null>(null);

  interface MusicItem {
    name: string;
    url: string;
    category?: string;
    source?: string;
    plugin_id?: string | null;
  }

  const musicList = ref<MusicItem[]>([]);

  // 音乐分类（子文件夹 = 子分类）
  const musicCategories = ref<string[]>([]);
  const currentMusicCategory = ref<string>("全部");
  const newMusicCategoryName = ref("");
  const VIRTUAL_CATEGORY = "插件";
  const isMusicCategoryReadOnly = computed(() => currentMusicCategory.value === VIRTUAL_CATEGORY);

  // 音乐收藏（localStorage 置顶）
  const MUSIC_FAVORED_KEY = "lingchat.music.favored.v1";
  const musicFavored = ref<string[]>([]);
  const isMusicFavored = (url: string): boolean => musicFavored.value.includes(url);
  function handleToggleMusicFavorite(url: string): void {
    if (musicFavored.value.includes(url)) {
      musicFavored.value = musicFavored.value.filter((u) => u !== url);
    } else {
      musicFavored.value.push(url);
    }
    localStorage.setItem(MUSIC_FAVORED_KEY, JSON.stringify(musicFavored.value));
  }

  // 根据分类过滤音乐列表，并按收藏置顶排序
  const filteredMusicList = computed(() => {
    const base =
      !currentMusicCategory.value || currentMusicCategory.value === "全部"
        ? musicList.value
        : musicList.value.filter((m) => m.category === currentMusicCategory.value);
    const favored = [...base].filter((m) => isMusicFavored(m.url));
    const unfavored = [...base].filter((m) => !isMusicFavored(m.url));
    return [...favored, ...unfavored];
  });

  // 批量上传状态
  const selectedPaths = ref<string[]>([]);

  // 播放模式设定 (loop-list: 列表循环, loop-single: 单曲循环, random: 随机)
  type PlaybackMode = "loop-list" | "loop-single" | "random";
  const modeText = computed(() => ({
    "loop-list": t("settings.sound.bgm.mode.loopList"),
    "loop-single": t("settings.sound.bgm.mode.loopSingle"),
    random: t("settings.sound.bgm.mode.random"),
  }));

  // 播放模式切换逻辑
  const togglePlaybackMode = () => {
    const modes: PlaybackMode[] = ["loop-list", "loop-single", "random"];
    const currentIndex = modes.indexOf(uiStore.bgMusicMode);
    const choice = modes[(currentIndex + 1) % modes.length];
    if (choice) uiStore.bgMusicMode = choice;
    else uiStore.bgMusicMode = "loop-list";
  };

  // 自动切歌处理 (响应播放结束事件)
  const handleTrackEnd = () => {
    if (musicList.value.length === 0) return;

    const currentUrl = uiStore.currentBackgroundMusic;
    const currentIndex = musicList.value.findIndex((m) => m.url === currentUrl);

    let nextMusic: MusicItem | undefined = undefined;

    if (uiStore.bgMusicMode === "loop-single") {
      // 单曲循环
      nextMusic = currentIndex !== -1 ? musicList.value[currentIndex] : musicList.value[0];
    } else if (uiStore.bgMusicMode === "random") {
      // 随机播放
      const randomIndex = Math.floor(Math.random() * musicList.value.length);
      nextMusic = musicList.value[randomIndex];
    } else {
      // 列表循环
      const nextIndex = currentIndex !== -1 ? (currentIndex + 1) % musicList.value.length : 0;
      nextMusic = musicList.value[nextIndex];
    }

    if (nextMusic) {
      playMusic(nextMusic);
    }
  };

  const inferMusicNameFromUrl = (musicUrl: string): string => {
    if (!musicUrl || musicUrl === "None") return t("settings.sound.bgm.noMusicSelected");
    const fileName = decodeURIComponent(musicUrl.split("/").pop() || "");
    if (!fileName) return t("settings.sound.bgm.noMusicSelected");
    return fileName.replace(/\.[^/.]+$/, "") || fileName;
  };

  // 当前音乐名：computed 依赖 uiStore.currentBackgroundMusic / musicList / locale，
  // 切歌、删歌或切换语言时会自动重新求值（无需手动同步）
  const currentMusicName = computed(() => {
    const currentUrl = uiStore.currentBackgroundMusic;
    if (!currentUrl || currentUrl === "None") return t("settings.sound.bgm.noMusicSelected");
    const matched = musicList.value.find((item) => item.url === currentUrl);
    return matched?.name || inferMusicNameFromUrl(currentUrl);
  });

  // 音量更新逻辑
  const updateCharacterVolume = (value: number) => {
    settingsStore.update("audio.characterVolume", value);
    if (characterTestPlayer.value) characterTestPlayer.value.volume = value / 100;
  };

  const updateBubbleVolume = (value: number) => {
    settingsStore.update("audio.bubbleVolume", value);
    if (bubbleTestPlayer.value) bubbleTestPlayer.value.volume = value / 100;
  };

  const updateBackgroundVolume = (value: number) => {
    settingsStore.update("audio.backgroundVolume", value);
    if (backgroundAudioPlayer.value) backgroundAudioPlayer.value.volume = value / 100;
  };

  const updateAchievementVolume = (value: number) => {
    settingsStore.update("audio.achievementVolume", value);
    if (achievementTestPlayer.value) achievementTestPlayer.value.volume = value / 100;
  };
  // 更新环境音音量（全局控制所有环境音轨道）
  const updateAmbientVolume = (value: number) => {
    settingsStore.update("audio.ambientVolume", value);
  };

  // ========== 环境音管理 ==========
  const maxAmbientTracks = 8;

  // 环境音文件库条目（服务端存储）
  const ambientFileList = ref<AmbientItem[]>([]);
  const selectedAmbientPaths = ref<string[]>([]);

  // 从文件URL推断显示名称
  const inferTrackName = (src: string): string => {
    if (!src) return t("settings.sound.ambient.unknownName");
    try {
      // convertFileSrc 生成的 URL 取最后一段
      const parts = src.split(/[/\\]/);
      const name = parts[parts.length - 1] || t("settings.sound.ambient.unknownName");
      return (
        decodeURIComponent(name.replace(/\.[^/.]+$/, "")) || t("settings.sound.ambient.unknownName")
      );
    } catch {
      return t("settings.sound.ambient.unknownName");
    }
  };

  // 停止单条环境音轨道
  const stopAmbientTrack = (id: string) => {
    uiStore.removeAmbientTrack(id);
  };

  // 停止所有环境音
  const stopAllAmbient = () => {
    uiStore.clearAmbientTracks();
  };

  // 触发环境音文件选择
  // 打开系统文件对话框选择环境音（拿路径，避免读文件进内存）
  const triggerAmbientUpload = async () => {
    const selected = await openDialog({
      multiple: true,
      filters: musicDialogFilters(),
    });
    if (!selected) return;
    selectedAmbientPaths.value = extractDialogPaths(selected);
  };

  /**
   * 从 openDialog 的返回中提取文件路径列表。
   *
   * 桌面端返回：string（单选）或 string[]（多选）
   * Android 返回：{ files: string[] }（SAF content:// URI）
   * 兼容两种格式，过滤空值。
   */
  const extractDialogPaths = (selected: unknown): string[] => {
    const raw = Array.isArray(selected)
      ? selected
      : selected && typeof selected === "object" && "files" in (selected as any)
        ? (selected as any).files
        : [selected];
    return raw
      .map((p: any) => (typeof p === "string" ? p : p?.path))
      .filter((p: any) => typeof p === "string" && p.length > 0);
  };

  // 从服务端加载环境音列表
  const loadAmbientList = async () => {
    try {
      ambientFileList.value = await ambientGetAll();
    } catch (e) {
      console.error("加载环境音列表失败:", e);
    }
  };

  // 确认上传已选环境音文件到服务端
  const uploadAmbientFiles = async () => {
    if (selectedAmbientPaths.value.length === 0) {
      await dialogStore.alert(t("settings.sound.ambient.selectFilesFirst"));
      return;
    }
    const allowedExts = [".mp3", ".wav", ".flac", ".ogg"];
    try {
      // 串行上传（仅传源文件路径，Rust 侧复制）
      for (const path of selectedAmbientPaths.value) {
        // content:// URI 文件名是 URL 编码的，解码后才是真实文件名
        const fileName = decodePathFileName(path);
        const fileExt = fileName.slice(fileName.lastIndexOf(".")).toLowerCase();
        if (!allowedExts.includes(fileExt))
          throw new Error(t("settings.sound.common.unsupportedFormat", { name: fileName }));
        await ambientUpload(path, fileName);
      }
      selectedAmbientPaths.value = [];
      await loadAmbientList();
    } catch (error: any) {
      console.error("上传环境音失败:", error);
      const rawMsg = error.message || String(error);
      const translated =
        rawMsg === "MUSIC_INVALID_FORMAT"
          ? t("ui.musicImport.errors.MUSIC_INVALID_FORMAT")
          : rawMsg;
      await dialogStore.alert(translated || t("settings.sound.ambient.uploadFailed"));
    }
  };

  // 从文件库添加到播放轨道
  const addFileToTrack = (ambient: AmbientItem) => {
    uiStore.addAmbientTrack({
      src: ambient.url,
      name: ambient.name,
      volume: 80,
      loop: true,
      fade: true,
    });
  };

  // 从服务端删除环境音文件
  const removeAmbientFile = async (ambient: AmbientItem) => {
    if (
      !(await dialogStore.confirm(
        t("settings.sound.ambient.confirmDelete", { name: ambient.name })
      ))
    )
      return;
    try {
      await ambientDelete(ambient.url);
      // 同时移除使用该文件的活跃轨道
      const tracksToRemove = uiStore.ambientTracks.filter((tr) => tr.src === ambient.url);
      for (const track of tracksToRemove) {
        uiStore.removeAmbientTrack(track.id);
      }
      await loadAmbientList();
    } catch (error: any) {
      console.error("删除环境音失败:", error);
      await dialogStore.alert(t("settings.sound.ambient.deleteFailed"));
    }
  };

  // 获取轨道显示名称（优先使用 name 字段，回退到路径推断）
  const getTrackDisplayName = (track: { name?: string; src: string }): string => {
    if (track.name) return track.name;
    return inferTrackName(track.src);
  };

  watch(
    () => settingsStore.backgroundVolume,
    (newVolume) => {
      if (backgroundAudioPlayer.value) backgroundAudioPlayer.value.volume = newVolume / 100;
    }
  );

  // currentMusicName 已改为 computed，自动依赖 uiStore.currentBackgroundMusic，无需手动 watch 同步

  // 监听播放器状态控制本地播放器
  watch(
    () => uiStore.bgMusicPaused,
    (isPaused) => {
      if (!backgroundAudioPlayer.value || !backgroundAudioPlayer.value.src) return;
      if (isPaused) {
        backgroundAudioPlayer.value.pause();
      } else {
        backgroundAudioPlayer.value.play().catch((e) => console.error("播放失败:", e));
      }
    }
  );

  // 监听背景音乐结束事件，通过store中的_musicEndTime触发
  watch(
    () => uiStore._musicEndTime,
    () => {
      // 当音乐结束时，调用handleTrackEnd处理音乐切换
      handleTrackEnd();
    }
  );

  const playCharacterTestSound = () => {
    if (!characterTestPlayer.value) return;
    characterTestPlayer.value.src = "/audio_effects/角色音量测试.wav";
    characterTestPlayer.value.play().catch((e) => console.error("测试角色音量播放失败:", e));
  };

  const playBubbleTestSound = () => {
    if (!bubbleTestPlayer.value) return;
    bubbleTestPlayer.value.src = "/audio_effects/疑问.wav";
    bubbleTestPlayer.value.play().catch((e) => console.error("测试气泡音量播放失败:", e));
  };

  const playAchievementTestSound = () => {
    if (!achievementTestPlayer.value) return;
    achievementTestPlayer.value.src = "/audio_effects/achievement_common.wav";
    achievementTestPlayer.value.play().catch((e) => console.error("测试成就音量播放失败:", e));
  };

  const loadMusicList = async () => {
    musicList.value = await musicGetAll();
  };

  // 加载音乐分类列表
  const loadMusicCategories = async () => {
    const cats = await musicListCategories();
    musicCategories.value = cats;
    if (currentMusicCategory.value !== "全部" && !cats.includes(currentMusicCategory.value)) {
      currentMusicCategory.value = "全部";
    }
  };

  // 新建音乐分类
  const handleCreateMusicCategory = async () => {
    const name = newMusicCategoryName.value.trim();
    if (!name) return;
    try {
      await musicCreateCategory(name);
      newMusicCategoryName.value = "";
      await loadMusicCategories();
      await loadMusicList();
    } catch (error: any) {
      dialogStore.alert(typeof error === "string" ? error : error.message || "创建分类失败");
    }
  };

  // 删除当前选中的音乐分类（音乐移到根目录）
  const handleDeleteMusicCategory = async () => {
    const cat = currentMusicCategory.value;
    if (cat === "全部" || cat === VIRTUAL_CATEGORY) return;
    if (
      !(await dialogStore.confirm(t("settings.sound.bgm.categoryDeleteConfirmMove", { name: cat })))
    )
      return;
    try {
      await musicDeleteCategory(cat, "move");
      currentMusicCategory.value = "全部";
      await loadMusicCategories();
      await loadMusicList();
    } catch (error: any) {
      dialogStore.alert(typeof error === "string" ? error : error.message || "删除分类失败");
    }
  };

  // 刷新音乐与分类
  const handleRefreshMusic = async () => {
    await loadMusicCategories();
    await loadMusicList();
  };

  // 打开音乐所在文件夹
  const handleOpenMusicFolder = async () => {
    try {
      await openMusicFolder();
    } catch (error: any) {
      dialogStore.alert(typeof error === "string" ? error : error.message || "打开文件夹失败");
    }
  };

  const deleteMusic = async (music: MusicItem) => {
    if (!music) return;
    if (!(await dialogStore.confirm(t("settings.sound.bgm.confirmDelete", { name: music.name }))))
      return;

    try {
      await musicDelete(music.url);
      const deletedMusicUrl = music.url;

      if (uiStore.currentBackgroundMusic === deletedMusicUrl) {
        uiStore.currentBackgroundMusic = "None";
        await setCurrentBackgroundMusic("None");

        if (backgroundAudioPlayer.value) {
          backgroundAudioPlayer.value.pause();
          backgroundAudioPlayer.value.currentTime = 0;
          backgroundAudioPlayer.value.src = "";
        }
        uiStore.bgMusicPaused = true;
      }
      await loadMusicList();
    } catch (error) {
      console.error("删除音乐失败:", error);
      await dialogStore.alert(t("settings.sound.bgm.deleteFailed"));
    }
  };

  // 批量上传逻辑
  const uploadMusic = async () => {
    if (isMusicCategoryReadOnly.value) return;
    if (selectedPaths.value.length === 0) {
      await dialogStore.alert(t("settings.sound.bgm.selectFilesFirst"));
      return;
    }

    try {
      // 串行上传（仅传源文件路径，Rust 侧复制 + magic 校验）
      for (const path of selectedPaths.value) {
        // content:// URI 文件名是 URL 编码的，解码后才是真实文件名
        const fileName = decodePathFileName(path);
        const upCat =
          currentMusicCategory.value === "全部" ? undefined : currentMusicCategory.value;
        const result = await musicUpload(path, fileName, upCat);
        // 自动修正时弹顶部 amber notice
        if (result.was_corrected) {
          const originalExt = result.original_name.split(".").pop() || "";
          roleStore.showCorrected({
            title: t("ui.notice.autoCorrected.title"),
            message: t("ui.notice.autoCorrected.music", {
              original: result.original_name,
              originalExt,
              detected: result.detected_kind,
              corrected: result.actual_name,
            }),
          });
        }
      }

      selectedPaths.value = [];
      await loadMusicList();
      await loadMusicCategories();
    } catch (error: any) {
      console.error("批量上传音乐出现问题:", error);
      const rawMsg = error.message || String(error);
      const translated =
        rawMsg === "MUSIC_INVALID_FORMAT"
          ? t("ui.musicImport.errors.MUSIC_INVALID_FORMAT")
          : rawMsg;
      await dialogStore.alert(translated || t("settings.sound.bgm.uploadFailed"));
    }
  };

  const playPauseButtonText = computed(() =>
    !uiStore.bgMusicPaused ? t("settings.sound.bgm.pause") : t("settings.sound.bgm.play")
  );

  const playMusic = async (music: MusicItem) => {
    let musicUrl = music.url;

    // 单曲循环的逻辑要更特殊一点
    // if (uiStore.bgMusicMode === 'loop-single') {
    //   musicUrl = uiStore.currentBackgroundMusic
    // }

    if (uiStore.currentBackgroundMusic === musicUrl) {
      uiStore.bgMusicPaused = false;
    }

    uiStore.currentBackgroundMusic = musicUrl;
    uiStore.bgMusicPaused = false;
    uiStore.bgMusicStoped = false;

    try {
      // await setCurrentBackgroundMusic(musicUrl)
    } catch (error) {
      console.error("保存背景音乐失败:", error);
    }
  };

  const handlePlayPause = () => {
    if (uiStore.currentBackgroundMusic === "None") return; // 未选曲目时不自动选中
    uiStore.bgMusicPaused = !uiStore.bgMusicPaused;
  };

  const handleStop = () => {
    uiStore.bgMusicStoped = true;
    uiStore.bgMusicPaused = true;
    uiStore.currentBackgroundMusic = "None";
    if (backgroundAudioPlayer.value) {
      backgroundAudioPlayer.value.currentTime = 0;
    }
  };

  // 打开系统文件对话框选择音乐（仅拿路径）
  const triggerFileUpload = async () => {
    if (isMusicCategoryReadOnly.value) return;
    const selected = await openDialog({
      multiple: true,
      filters: musicDialogFilters(),
    });
    if (!selected) return;
    selectedPaths.value = extractDialogPaths(selected);
  };

  onMounted(async () => {
    // 加载音乐收藏
    try {
      const raw = localStorage.getItem(MUSIC_FAVORED_KEY);
      musicFavored.value = raw ? JSON.parse(raw) : [];
    } catch {
      musicFavored.value = [];
    }
    await loadMusicList();
    await loadMusicCategories();
    await loadAmbientList();

    // 初始化音量
    if (characterTestPlayer.value) characterTestPlayer.value.volume = characterVolume.value / 100;
    if (bubbleTestPlayer.value) bubbleTestPlayer.value.volume = bubbleVolume.value / 100;
    if (achievementTestPlayer.value)
      achievementTestPlayer.value.volume = achievementVolume.value / 100;

    if (backgroundAudioPlayer.value) {
      backgroundAudioPlayer.value.volume = backgroundVolume.value / 100;
      if (uiStore.currentBackgroundMusic && uiStore.currentBackgroundMusic !== "None") {
        backgroundAudioPlayer.value.src = uiStore.currentBackgroundMusic;
        // 如果 Store 中的状态是播放，则尝试恢复播放
        if (!uiStore.bgMusicPaused) {
          backgroundAudioPlayer.value.play().catch((e) => console.warn("自动播放受限:", e));
        }
      }
    }
  });
</script>

<style>
  /* 原生 select 下拉弹层跟随深色主题，避免白底白字 */
  .output-device-select {
    color-scheme: dark;
  }
  /* 显式设置选项背景，兜底 color-scheme 未生效的情况 */
  .output-device-select option {
    background-color: #1e1e2e;
    color: #e2e8f0;
  }
  .output-device-select option:checked {
    background-color: #3b82f6;
    color: #ffffff;
  }

  /* 仅保留无法用简短 Tailwind 涵盖的自定义滚动条样式 */
  .custom-scrollbar::-webkit-scrollbar {
    width: 6px;
  }
  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background-color: rgba(255, 255, 255, 0.2);
    border-radius: 20px;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background-color: rgba(255, 255, 255, 0.4);
  }
</style>
