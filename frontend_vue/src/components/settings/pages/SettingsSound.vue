<template>
  <MenuPage>
    <!-- 音量控制部分 -->
    <MenuItem title="角色音量" size="small">
      <template #header>
        <MicVocal :size="20" class="text-indigo-400" />
      </template>
      <Slider v-model="characterVolume" @change="updateCharacterVolume"> 弱/强 </Slider>
    </MenuItem>

    <MenuItem title="气泡音量" size="small">
      <template #header>
        <MessageCircle :size="20" class="text-blue-400" />
      </template>
      <Slider @change="updateBubbleVolume" v-model="bubbleVolume"> 弱/强 </Slider>
    </MenuItem>

    <MenuItem title="背景音量" size="small">
      <template #header>
        <AudioLines :size="20" class="text-green-400" />
      </template>
      <Slider @change="updateBackgroundVolume" v-model="backgroundVolume"> 弱/强 </Slider>
    </MenuItem>

    <MenuItem title="成就音量" size="small">
      <template #header>
        <Trophy :size="20" class="text-yellow-400" />
      </template>
      <Slider @change="updateAchievementVolume" v-model="achievementVolume"> 弱/强 </Slider>
    </MenuItem>

    <!-- 测试声音部分 -->
    <MenuItem title="声音测试" size="small">
      <template #header>
        <FlaskConical :size="20" class="text-pink-400" />
      </template>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <Button type="big" class="flex-1 min-w-30" @click="playCharacterTestSound">测试角色</Button>
        <Button type="big" class="flex-1 min-w-30" @click="playBubbleTestSound">测试气泡</Button>
        <Button type="big" class="flex-1 min-w-30" @click="playAchievementTestSound"
          >测试成就</Button
        >
      </div>
    </MenuItem>

    <!-- 背景音乐设置部分 -->
    <MenuItem title="背景音乐设置">
      <template #header>
        <Headphones :size="20" class="text-purple-400" />
      </template>

      <!-- 音乐控制台 -->
      <div class="flex gap-3 bg-white/5 border border-white/10 rounded-xl p-4 backdrop-blur-md">
        <div
          class="flex w-[60%] items-center justify-between text-sm font-medium text-gray-200 bg-black/20 px-3 py-2 rounded-lg"
        >
          <span class="flex items-center gap-2 truncate">
            <Music :size="16" class="text-purple-400 shrink-0" />
            <span class="truncate">{{ currentMusicName }}</span>
          </span>
          <span class="text-xs text-gray-400 shrink-0 ml-2">{{ modeText[playbackMode] }}</span>
        </div>

        <div class="flex w-[40%] items-center gap-2">
          <Button
            type="big"
            @click="handlePlayPause"
            class="flex justify-center items-center gap-1"
          >
            <Play v-if="uiStore.bgMusicPaused" :size="16" />
            <Pause v-else :size="16" />
            {{ playPauseButtonText }}
          </Button>
          <Button type="big" @click="handleStop" class="flex justify-center items-center gap-1">
            <Square :size="14" /> 停止
          </Button>
          <Button
            type="big"
            @click="togglePlaybackMode"
            class="flex justify-center items-center"
            :title="modeText[playbackMode]"
          >
            <Repeat1 v-if="playbackMode === 'loop-single'" :size="18" />
            <Repeat v-else-if="playbackMode === 'loop-list'" :size="18" />
            <Shuffle v-else :size="18" />
          </Button>
        </div>
      </div>

      <!-- 音乐列表 -->
      <div
        class="mt-4 border border-white/10 rounded-xl bg-black/20 backdrop-blur-sm overflow-hidden flex flex-col"
      >
        <div v-if="musicList.length === 0" class="text-center text-gray-400 py-8 text-sm">
          暂无音乐，请在下方上传
        </div>
        <div v-else class="max-h-52 overflow-y-auto p-1.5 space-y-1 custom-scrollbar">
          <div
            v-for="music in musicList"
            :key="music.url"
            @click="playMusic(music)"
            class="group flex justify-between items-center px-3 py-2.5 cursor-pointer rounded-lg transition-all duration-200 hover:bg-white/10"
            :class="{ 'bg-purple-500/20 text-purple-300': currentMusicName === music.name }"
          >
            <div
              class="flex-1 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium pr-2"
            >
              {{ music.name }}
            </div>
            <button
              @click.stop="deleteMusic(music)"
              class="opacity-0 group-hover:opacity-100 transition-opacity duration-200 p-1.5 rounded-md bg-red-500/10 hover:bg-red-500/80 text-red-400 hover:text-white"
              title="删除"
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
          class="flex-1 flex justify-center items-center gap-2"
        >
          <UploadCloud :size="18" /> 添加音乐
        </Button>
        <!-- 修改为支持 multiple 多选 -->
        <input
          ref="fileInput"
          type="file"
          multiple
          @change="handleFileSelect"
          accept=".mp3,.wav,.flac,.webm,.weba,.ogg,.m4a"
          class="hidden"
        />
        <div class="flex-1 flex items-center justify-between gap-2">
          <span class="text-xs text-gray-400 truncate w-24" v-if="selectedFiles.length > 0">
            已选 {{ selectedFiles.length }} 个文件
          </span>
          <span class="text-xs text-gray-500 truncate w-24" v-else>未选择文件</span>

          <Button
            type="big"
            @click="uploadMusic"
            :disabled="selectedFiles.length === 0"
            class="flex-1"
            :class="{ 'opacity-50 cursor-not-allowed': selectedFiles.length === 0 }"
          >
            确认上传
          </Button>
        </div>
      </div>
    </MenuItem>

    <!-- 音频播放器 (隐藏) -->
    <audio ref="characterTestPlayer"></audio>
    <audio ref="bubbleTestPlayer"></audio>
    <audio ref="achievementTestPlayer"></audio>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Button, Slider } from '../../base'
import { MenuItem, MenuPage } from '../../ui'
import {
  musicDelete,
  musicGetAll,
  musicUpload,
  setCurrentBackgroundMusic,
} from '../../../api/services/music'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useSettingsStore } from '../../../stores/modules/settings'
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
} from 'lucide-vue-next'

const uiStore = useUIStore()
const settingsStore = useSettingsStore()

// 状态绑定
const characterVolume = computed({
  get: () => settingsStore.characterVolume,
  set: (val: number) => settingsStore.update('audio.characterVolume', val),
})
const bubbleVolume = computed({
  get: () => settingsStore.bubbleVolume,
  set: (val: number) => settingsStore.update('audio.bubbleVolume', val),
})
const backgroundVolume = computed({
  get: () => settingsStore.backgroundVolume,
  set: (val: number) => settingsStore.update('audio.backgroundVolume', val),
})
const achievementVolume = computed({
  get: () => settingsStore.achievementVolume,
  set: (val: number) => settingsStore.update('audio.achievementVolume', val),
})

// 音频引用
const characterTestPlayer = ref<HTMLAudioElement | null>(null)
const bubbleTestPlayer = ref<HTMLAudioElement | null>(null)
const achievementTestPlayer = ref<HTMLAudioElement | null>(null)
const backgroundAudioPlayer = ref<HTMLAudioElement | null>(null)

interface MusicItem {
  name: string
  url: string
}

const musicList = ref<MusicItem[]>([])
const currentMusicName = ref('未选择音乐')

// 批量上传状态
const selectedFiles = ref<File[]>([])
const fileInput = ref<HTMLInputElement | null>(null)

// 播放模式设定 (loop-list: 列表循环, loop-single: 单曲循环, random: 随机)
type PlaybackMode = 'loop-list' | 'loop-single' | 'random'
const playbackMode = ref<PlaybackMode>('loop-list')
const modeText = {
  'loop-list': '列表循环',
  'loop-single': '单曲循环',
  random: '随机播放',
}

// 播放模式切换逻辑
const togglePlaybackMode = () => {
  const modes: PlaybackMode[] = ['loop-list', 'loop-single', 'random']
  const currentIndex = modes.indexOf(playbackMode.value)
  playbackMode.value = modes[(currentIndex + 1) % modes.length]
}

// 自动切歌处理 (响应播放结束事件)
const handleTrackEnd = () => {
  if (musicList.value.length === 0) return

  const currentUrl = uiStore.currentBackgroundMusic
  const currentIndex = musicList.value.findIndex((m) => toMusicUrl(m.url) === currentUrl)

  let nextMusic: MusicItem | undefined = undefined

  if (playbackMode.value === 'loop-single') {
    // 单曲循环
    nextMusic = currentIndex !== -1 ? musicList.value[currentIndex] : musicList.value[0]
  } else if (playbackMode.value === 'random') {
    // 随机播放
    const randomIndex = Math.floor(Math.random() * musicList.value.length)
    nextMusic = musicList.value[randomIndex]
  } else {
    // 列表循环
    const nextIndex = currentIndex !== -1 ? (currentIndex + 1) % musicList.value.length : 0
    nextMusic = musicList.value[nextIndex]
  }

  if (nextMusic) {
    playMusic(nextMusic)
  }
}

const toMusicUrl = (musicFileName: string): string =>
  `/api/v1/chat/back-music/music_file/${encodeURIComponent(musicFileName)}`

const inferMusicNameFromUrl = (musicUrl: string): string => {
  if (!musicUrl || musicUrl === 'None') return '未选择音乐'
  const fileName = decodeURIComponent(musicUrl.split('/').pop() || '')
  if (!fileName) return '未选择音乐'
  return fileName.replace(/\.[^/.]+$/, '') || fileName
}

const syncCurrentMusicName = () => {
  const currentUrl = uiStore.currentBackgroundMusic
  if (!currentUrl || currentUrl === 'None') {
    currentMusicName.value = '未选择音乐'
    return
  }
  const matched = musicList.value.find((item) => toMusicUrl(item.url) === currentUrl)
  currentMusicName.value = matched?.name || inferMusicNameFromUrl(currentUrl)
}

// 音量更新逻辑
const updateCharacterVolume = (value: number) => {
  settingsStore.update('audio.characterVolume', value)
  if (characterTestPlayer.value) characterTestPlayer.value.volume = value / 100
}

const updateBubbleVolume = (value: number) => {
  settingsStore.update('audio.bubbleVolume', value)
  if (bubbleTestPlayer.value) bubbleTestPlayer.value.volume = value / 100
}

const updateBackgroundVolume = (value: number) => {
  settingsStore.update('audio.backgroundVolume', value)
  if (backgroundAudioPlayer.value) backgroundAudioPlayer.value.volume = value / 100
}

const updateAchievementVolume = (value: number) => {
  settingsStore.update('audio.achievementVolume', value)
  if (achievementTestPlayer.value) achievementTestPlayer.value.volume = value / 100
}

watch(
  () => settingsStore.backgroundVolume,
  (newVolume) => {
    if (backgroundAudioPlayer.value) backgroundAudioPlayer.value.volume = newVolume / 100
  },
)

watch(
  () => uiStore.currentBackgroundMusic,
  (newUrl) => {
    syncCurrentMusicName()
    // 确保本地播放器的 URL 实时跟随 Store
  },
)

// 监听播放器状态控制本地播放器
watch(
  () => uiStore.bgMusicPaused,
  (isPaused) => {
    if (!backgroundAudioPlayer.value || !backgroundAudioPlayer.value.src) return
    if (isPaused) {
      backgroundAudioPlayer.value.pause()
    } else {
      backgroundAudioPlayer.value.play().catch((e) => console.error('播放失败:', e))
    }
  },
)

// 监听背景音乐结束事件，通过store中的_musicEndTime触发
watch(
  () => uiStore._musicEndTime,
  () => {
    // 当音乐结束时，调用handleTrackEnd处理音乐切换
    handleTrackEnd()
  },
)

const playCharacterTestSound = () => {
  if (!characterTestPlayer.value) return
  characterTestPlayer.value.src = '/audio_effects/角色音量测试.wav'
  characterTestPlayer.value.play().catch((e) => console.error('测试角色音量播放失败:', e))
}

const playBubbleTestSound = () => {
  if (!bubbleTestPlayer.value) return
  bubbleTestPlayer.value.src = '/audio_effects/疑问.wav'
  bubbleTestPlayer.value.play().catch((e) => console.error('测试气泡音量播放失败:', e))
}

const playAchievementTestSound = () => {
  if (!achievementTestPlayer.value) return
  achievementTestPlayer.value.src = '/audio_effects/achievement_common.wav'
  achievementTestPlayer.value.play().catch((e) => console.error('测试成就音量播放失败:', e))
}

const loadMusicList = async () => {
  musicList.value = await musicGetAll()
  syncCurrentMusicName()
}

const deleteMusic = async (music: MusicItem) => {
  if (!music) return
  if (!confirm(`确定要删除《${music.name}》吗？`)) return

  try {
    await musicDelete(music.url)
    const deletedMusicUrl = toMusicUrl(music.url)

    if (uiStore.currentBackgroundMusic === deletedMusicUrl) {
      uiStore.currentBackgroundMusic = 'None'
      currentMusicName.value = '未选择音乐'
      await setCurrentBackgroundMusic('None')

      if (backgroundAudioPlayer.value) {
        backgroundAudioPlayer.value.pause()
        backgroundAudioPlayer.value.currentTime = 0
        backgroundAudioPlayer.value.src = ''
      }
      uiStore.bgMusicPaused = true
    }
    await loadMusicList()
  } catch (error) {
    console.error('删除音乐失败:', error)
    alert('删除音乐失败')
  }
}

// 批量上传逻辑
const uploadMusic = async () => {
  if (selectedFiles.value.length === 0) {
    alert('请先选择音乐文件')
    return
  }

  const allowedExts = ['.mp3', '.wav', '.flac', '.webm', '.weba', '.ogg', '.m4a']

  try {
    // 使用 Promise.all 进行并发上传
    await Promise.all(
      selectedFiles.value.map(async (file) => {
        const fileExt = file.name.slice(file.name.lastIndexOf('.')).toLowerCase()
        if (!allowedExts.includes(fileExt)) {
          throw new Error(`格式不支持: ${file.name}`)
        }
        const formData = new FormData()
        formData.append('file', file)
        await musicUpload(formData)
      }),
    )

    selectedFiles.value = []
    if (fileInput.value) fileInput.value.value = ''
    await loadMusicList()
    // alert('音乐上传成功') // 可选提示
  } catch (error: any) {
    console.error('批量上传音乐出现问题:', error)
    alert(error.message || '部分或全部音乐上传失败')
  }
}

const playPauseButtonText = computed(() => (!uiStore.bgMusicPaused ? '暂停' : '播放'))

const playMusic = async (music: MusicItem) => {
  const musicUrl = toMusicUrl(music.url)
  currentMusicName.value = music.name

  if (uiStore.currentBackgroundMusic === musicUrl) {
    uiStore.bgMusicPaused = false
  }

  uiStore.currentBackgroundMusic = musicUrl

  try {
    await setCurrentBackgroundMusic(musicUrl)
    uiStore.bgMusicPaused = false
  } catch (error) {
    console.error('保存背景音乐失败:', error)
  }
}

const handlePlayPause = () => {
  if (uiStore.currentBackgroundMusic === 'None' && musicList.value.length > 0) {
    // 如果还没选过歌，默认播放第一首
    const music = musicList.value[0]
    if (music) playMusic(music)
  } else {
    uiStore.bgMusicPaused = !uiStore.bgMusicPaused
  }
}

const handleStop = () => {
  uiStore.bgMusicStoped = true
  uiStore.bgMusicPaused = true
  if (backgroundAudioPlayer.value) {
    backgroundAudioPlayer.value.currentTime = 0
  }
}

const triggerFileUpload = () => {
  fileInput.value?.click()
}

// 处理多文件选择
const handleFileSelect = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    selectedFiles.value = Array.from(target.files)
  } else {
    selectedFiles.value = []
  }
}

onMounted(async () => {
  await loadMusicList()

  // 初始化音量
  if (characterTestPlayer.value) characterTestPlayer.value.volume = characterVolume.value / 100
  if (bubbleTestPlayer.value) bubbleTestPlayer.value.volume = bubbleVolume.value / 100
  if (achievementTestPlayer.value)
    achievementTestPlayer.value.volume = achievementVolume.value / 100

  if (backgroundAudioPlayer.value) {
    backgroundAudioPlayer.value.volume = backgroundVolume.value / 100
    if (uiStore.currentBackgroundMusic && uiStore.currentBackgroundMusic !== 'None') {
      backgroundAudioPlayer.value.src = uiStore.currentBackgroundMusic
      // 如果 Store 中的状态是播放，则尝试恢复播放
      if (!uiStore.bgMusicPaused) {
        backgroundAudioPlayer.value.play().catch((e) => console.warn('自动播放受限:', e))
      }
    }
  }

  syncCurrentMusicName()
})
</script>

<style>
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
