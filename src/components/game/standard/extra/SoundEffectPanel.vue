<template>
  <div class="relative pointer-events-auto">
    <!-- 底部左侧触发按钮（用定位容器包裹，避免 .nav 的 position:relative 覆盖 fixed） -->
    <div
      v-if="!uiStore.showSettings"
      class="fixed bottom-6 left-6 z-[1000]"
    >
      <Button
        type="nav"
        :icon="'sound'"
        ref="triggerRef"
        :class="[
          'flex items-center gap-2 px-4 py-2 transition-colors',
          hasActiveAudio ? 'text-[#4facfe] pulse-icon' : 'text-white',
        ]"
        @click="panelVisible = !panelVisible"
      >
        <h3 class="text-lg font-bold m-0 hidden xl:block">声效</h3>
      </Button>
    </div>

    <!-- 弹出面板（复用日程/番茄钟面板样式） -->
    <Transition
      enter-active-class="transition-all duration-300"
      leave-active-class="transition-all duration-200"
      enter-from-class="opacity-0 -translate-y-2 scale-95"
      leave-to-class="opacity-0 -translate-y-2 scale-95"
    >
      <div
        v-if="panelVisible"
        ref="panelRef"
        class="fixed bottom-16 left-4 w-[520px] max-h-[80vh] overflow-y-auto bg-[#12121c]/75 backdrop-blur-[20px] border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)] rounded-3xl p-4 text-white box-border z-[1000] custom-scrollbar"
      >
        <!-- ===== BGM 区域 ===== -->
        <div class="mb-4">
          <div class="flex items-center justify-between mb-3">
            <span class="text-sm font-semibold text-gray-300 flex items-center gap-2">
              <Music2 :size="14" class="text-[#79d9ff]" />
              背景音乐
            </span>
            <div class="flex items-center gap-1">
              <button
                @click="handlePlayPause"
                class="p-1.5 rounded-lg text-gray-400 hover:text-white hover:bg-white/10 transition-colors"
                :title="uiStore.bgMusicPaused ? '播放' : '暂停'"
              >
                <Play v-if="uiStore.bgMusicPaused" :size="14" />
                <Pause v-else :size="14" />
              </button>
              <button
                @click="handleStop"
                class="p-1.5 rounded-lg text-gray-400 hover:text-white hover:bg-white/10 transition-colors"
                title="停止"
              >
                <Square :size="13" />
              </button>
              <button
                @click="togglePlaybackMode"
                class="p-1.5 rounded-lg text-gray-400 hover:text-white hover:bg-white/10 transition-colors"
                :title="modeText[uiStore.bgMusicMode]"
              >
                <Repeat v-if="uiStore.bgMusicMode === 'loop-list'" :size="13" />
                <Repeat1 v-else-if="uiStore.bgMusicMode === 'loop-single'" :size="13" />
                <Shuffle v-else :size="13" />
              </button>
            </div>
          </div>

          <!-- 当前曲名 + 模式 -->
          <div class="text-xs text-gray-400 mb-2 px-1 truncate">
            {{ currentMusicName }}
            <span v-if="uiStore.bgMusicMode" class="ml-2 text-gray-500">
              {{ modeText[uiStore.bgMusicMode] }}
            </span>
          </div>

          <!-- BGM 音量滑块 -->
          <div class="flex items-center gap-2 px-1 mb-2">
            <Volume2 :size="12" class="text-gray-500 shrink-0" />
            <input
              type="range"
              min="0"
              max="100"
              :value="settingsStore.audio.backgroundVolume"
              @input="(e) => settingsStore.audio.backgroundVolume = Number((e.target as HTMLInputElement).value)"
              class="flex-1 h-1 accent-[#79d9ff] bg-white/10 rounded-full appearance-none cursor-pointer"
            />
            <span class="text-[10px] text-gray-500 w-6 text-right shrink-0 tabular-nums">{{ settingsStore.audio.backgroundVolume }}</span>
          </div>

          <!-- BGM 曲目列表（始终可见，可滚动） -->
          <div
            class="border border-white/5 rounded-xl bg-white/[0.03] overflow-hidden"
          >
            <div v-if="bgmList.length === 0" class="text-center text-gray-500 py-4 text-xs">
              暂无音乐
            </div>
            <div v-else class="max-h-32 overflow-y-auto p-1 space-y-0.5 custom-scrollbar">
              <div
                v-for="music in bgmList"
                :key="music.url"
                @click="playMusic(music)"
                class="group flex items-center gap-2 px-3 py-1.5 rounded-lg cursor-pointer transition-all duration-150"
                :class="[
                  uiStore.currentBackgroundMusic === music.url
                    ? 'bg-[#79d9ff]/20 text-[#79d9ff]'
                    : 'hover:bg-white/10 text-gray-400 hover:text-white'
                ]"
              >
                <span class="flex-1 text-xs truncate">{{ music.name }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- 分隔线 -->
        <div class="h-px bg-white/5 mb-4"></div>

        <!-- ===== 环境音区域 ===== -->
        <div>
          <div class="flex items-center justify-between mb-3">
            <span class="text-sm font-semibold text-gray-300 flex items-center gap-2">
              <Wind :size="14" class="text-[#79d9ff]" />
              环境音
              <span class="text-xs text-gray-500 font-normal">({{ uiStore.ambientTracks.length }})</span>
            </span>
            <button
              v-if="uiStore.ambientTracks.length > 0"
              @click="stopAllAmbient"
              class="text-xs text-red-400 hover:text-red-300 transition-colors flex items-center gap-1 px-2 py-1 rounded hover:bg-red-500/10"
            >
              <Square :size="10" /> 全部停止
            </button>
          </div>

          <!-- 活跃轨道列表 -->
          <div v-if="uiStore.ambientTracks.length === 0" class="text-xs text-gray-500 text-center py-4 bg-white/[0.03] rounded-xl">
            没有正在播放的环境音
          </div>
          <div v-else class="space-y-2">
            <div
              v-for="track in uiStore.ambientTracks"
              :key="track.id"
              class="bg-white/[0.03] rounded-xl px-3 py-2 border border-white/5"
            >
              <!-- 轨道名 + 控制按钮 -->
              <div class="flex items-center gap-2 mb-2">
                <Wind :size="12" class="text-[#79d9ff] shrink-0" />
                <span class="flex-1 text-xs text-gray-200 truncate">{{ getTrackDisplayName(track) }}</span>
                <button
                  @click="uiStore.toggleAmbientTrackPause(track.id)"
                  class="p-1 rounded text-gray-400 hover:text-white hover:bg-white/10 transition-colors"
                  :title="track.paused ? '恢复' : '暂停'"
                >
                  <Play v-if="track.paused" :size="11" />
                  <Pause v-else :size="11" />
                </button>
                <button
                  @click="uiStore.removeAmbientTrack(track.id)"
                  class="p-1 rounded text-gray-500 hover:text-red-400 hover:bg-red-500/10 transition-colors"
                  title="移除"
                >
                  <X :size="11" />
                </button>
              </div>
              <!-- 单轨音量滑块 -->
              <div class="flex items-center gap-2 pl-5">
                <Volume2 :size="10" class="text-gray-500 shrink-0" />
                <input
                  type="range"
                  min="0"
                  max="100"
                  :value="track.volume"
                  @input="(e) => uiStore.updateAmbientTrackVolume(track.id, Number((e.target as HTMLInputElement).value))"
                  class="flex-1 h-1 accent-[#79d9ff] bg-white/10 rounded-full appearance-none cursor-pointer"
                />
                <span class="text-[10px] text-gray-500 w-6 text-right shrink-0 tabular-nums">{{ track.volume }}</span>
              </div>
            </div>
          </div>
          <!-- 环境音文件库（可选择播放） -->
          <div v-if="ambientFileList.length > 0" class="mt-3">
            <span class="text-xs text-gray-400 mb-1.5 block">可用环境音</span>
            <div class="border border-white/5 rounded-xl bg-white/[0.03] overflow-hidden">
              <div class="max-h-28 overflow-y-auto p-1 space-y-0.5 custom-scrollbar">
                <div
                  v-for="ambient in ambientFileList"
                  :key="ambient.url"
                  @click="playAmbientFromList(ambient)"
                  class="group flex items-center gap-2 px-3 py-1.5 rounded-lg cursor-pointer transition-all duration-150 hover:bg-white/10 text-gray-400 hover:text-white"
                >
                  <Wind :size="12" class="text-[#79d9ff] shrink-0" />
                  <span class="flex-1 text-xs truncate">{{ ambient.name }}</span>
                  <Play :size="10" class="opacity-0 group-hover:opacity-100 transition-opacity" />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import {
  Music2,
  Play,
  Pause,
  Square,
  Repeat,
  Repeat1,
  Shuffle,
  Wind,
  X,
  Volume2,
} from 'lucide-vue-next'
import { useUIStore } from '@/stores/modules/ui/ui'
import { useSettingsStore } from '@/stores/modules/settings'
import { musicGetAll } from '@/api/services/music'
import { ambientGetAll, type AmbientItem } from '@/api/services/ambient'
import Button from '@/components/base/widget/Button.vue'

const uiStore = useUIStore()
const settingsStore = useSettingsStore()

// ===== 面板状态 =====
const panelVisible = ref(false)
const panelRef = ref<HTMLElement | null>(null)

// ===== 是否有活跃音频在播放（控制图标闪烁，暂停/停止时不闪） =====
const hasActiveAudio = computed(() => {
  const bgmPlaying = uiStore.currentBackgroundMusic !== 'None' && !uiStore.bgMusicPaused && !uiStore.bgMusicStoped
  const ambientPlaying = uiStore.ambientTracks.length > 0 && uiStore.ambientTracks.some(t => !t.paused)
  return bgmPlaying || ambientPlaying
})

// ===== BGM 相关 =====
interface MusicItem {
  name: string
  url: string
}

const bgmList = ref<MusicItem[]>([])
const currentMusicName = ref('未选择音乐')

const modeText: Record<string, string> = {
  'loop-list': '列表循环',
  'loop-single': '单曲循环',
  random: '随机播放',
}

/** 从文件路径提取文件名 */
const extractFileName = (filePath: string): string => {
  if (!filePath || filePath === 'None') return '未选择音乐'
  const parts = filePath.replace(/\\/g, '/').split('/')
  const fileName = decodeURIComponent(parts.pop() || '')
  if (!fileName) return '未选择音乐'
  return fileName.replace(/\.[^/.]+$/, '') || fileName
}

/** 同步当前曲名 */
const syncCurrentMusicName = () => {
  const currentUrl = uiStore.currentBackgroundMusic
  if (!currentUrl || currentUrl === 'None') {
    currentMusicName.value = '未选择音乐'
    return
  }
  const matched = bgmList.value.find((item) => item.url === currentUrl)
  currentMusicName.value = matched?.name || extractFileName(currentUrl)
}

/** 切换播放模式 */
const togglePlaybackMode = () => {
  const modes: Array<'loop-list' | 'loop-single' | 'random'> = ['loop-list', 'loop-single', 'random']
  const currentIndex = modes.indexOf(uiStore.bgMusicMode)
  uiStore.bgMusicMode = modes[(currentIndex + 1) % modes.length]
}

/** 播放/暂停 */
const handlePlayPause = () => {
  if (uiStore.currentBackgroundMusic === 'None') return  // 未选曲目时不自动选中
  uiStore.bgMusicPaused = !uiStore.bgMusicPaused
}

/** 停止：清除选中状态，回退到"未选择音乐" */
const handleStop = () => {
  uiStore.bgMusicStoped = true
  uiStore.bgMusicPaused = true
  uiStore.currentBackgroundMusic = 'None'
}

/** 播放指定曲目 */
const playMusic = (music: MusicItem) => {
  uiStore.currentBackgroundMusic = music.url
  uiStore.bgMusicPaused = false
  uiStore.bgMusicStoped = false
}

// ===== 环境音相关 =====
/** 从 src 推断轨道显示名 */
const getTrackDisplayName = (track: { name?: string; src: string }): string => {
  if (track.name) return track.name
  return extractFileName(track.src)
}

/** 停止全部环境音 */
const stopAllAmbient = () => {
  uiStore.clearAmbientTracks()
}

// ===== 数据加载 =====
const loadBgmList = async () => {
  try {
    bgmList.value = await musicGetAll()
    syncCurrentMusicName()
  } catch (e) {
    console.error('SoundEffectPanel: 加载音乐列表失败', e)
  }
}

// ===== 点击外部关闭 =====
const triggerRef = ref<HTMLElement | null>(null)

// ===== 环境音文件库 =====
const ambientFileList = ref<AmbientItem[]>([])

const loadAmbientList = async () => {
  try {
    ambientFileList.value = await ambientGetAll()
  } catch (e) {
    console.error('SoundEffectPanel: 加载环境音列表失败', e)
  }
}

/** 从文件库播放环境音 */
const playAmbientFromList = (ambient: AmbientItem) => {
  uiStore.addAmbientTrack({
    src: ambient.url,
    name: ambient.name,
    volume: 80,
    loop: true,
    fade: true,
  })
}

const handleClickOutside = (e: MouseEvent) => {
  if (!panelVisible.value) return
  const target = e.target as Node
  // 点击在面板内部或触发按钮上都不关闭
  if (panelRef.value?.contains(target)) return
  if (triggerRef.value?.contains(target)) return
  panelVisible.value = false
}

// ===== 生命周期 =====
onMounted(() => {
  loadBgmList()
  loadAmbientList()
  document.addEventListener('mousedown', handleClickOutside)
})

onUnmounted(() => {
  document.removeEventListener('mousedown', handleClickOutside)
})

// 监听 BGM URL 变化同步名称
watch(
  () => uiStore.currentBackgroundMusic,
  () => syncCurrentMusicName(),
)

// 打开设置时自动关闭声效面板
watch(
  () => uiStore.showSettings,
  (val) => { if (val) panelVisible.value = false },
)
</script>

<style scoped>
/* 自定义滚动条 */
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background-color: rgba(255, 255, 255, 0.15);
  border-radius: 20px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background-color: rgba(255, 255, 255, 0.3);
}

/* 音量滑块样式 */
input[type='range']::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #79d9ff;
  cursor: pointer;
  border: 2px solid rgba(0, 0, 0, 0.3);
}

/* 图标脉冲动画（.pulse-icon 仅让第一个子元素即 Icon 闪烁，文字不闪） */
@keyframes pulse-glow {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
.pulse-icon > :deep(span:first-child) {
  animation: pulse-glow 2s ease-in-out infinite;
}
</style>
