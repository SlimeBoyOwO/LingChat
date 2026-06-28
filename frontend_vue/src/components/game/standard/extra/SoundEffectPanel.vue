<template>
  <!-- 触发按钮 -->
  <button
    class="sound-effect-trigger"
    :class="{ 'has-active': hasActiveAudio }"
    @click="togglePanel"
    title="音效控制"
  >
    <Music2 :size="20" />
  </button>

  <!-- 控制面板 -->
  <Transition
    enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
    leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
    enter-from-class="opacity-0 translate-y-2"
    leave-to-class="opacity-0 translate-y-2"
  >
    <div
      v-if="panelVisible"
      ref="panelRef"
      class="sound-effect-panel"
      data-wheel-history-ignore
      @wheel.stop
    >
      <!-- ====== BGM 区域 ====== -->
      <div class="panel-section">
        <div class="section-title">
          <Music :size="16" class="text-purple-400" />
          <span>背景音乐</span>
        </div>

        <!-- BGM 当前曲目 -->
        <div class="bgm-current">
          <span class="truncate">{{ currentMusicName }}</span>
          <span class="mode-badge">{{ modeText[uiStore.bgMusicMode] }}</span>
        </div>

        <!-- BGM 控制按钮组 -->
        <div class="control-row">
          <button class="ctrl-btn" @click="handlePlayPause" title="播放/暂停">
            <Play v-if="uiStore.bgMusicPaused" :size="16" />
            <Pause v-else :size="16" />
          </button>
          <button class="ctrl-btn" @click="handleStop" title="停止">
            <Square :size="14" />
          </button>
          <button class="ctrl-btn" @click="togglePlaybackMode" :title="modeText[uiStore.bgMusicMode]">
            <Repeat1 v-if="uiStore.bgMusicMode === 'loop-single'" :size="16" />
            <Repeat v-else-if="uiStore.bgMusicMode === 'loop-list'" :size="16" />
            <Shuffle v-else :size="16" />
          </button>
        </div>

        <!-- BGM 音量 -->
        <div class="volume-row">
          <Volume2 :size="14" class="text-gray-400 shrink-0" />
          <input
            type="range"
            min="0"
            max="100"
            :value="uiStore.backgroundVolume"
            @input="onBgmVolumeChange"
            class="volume-slider"
          />
        </div>
      </div>

      <!-- 分隔线 -->
      <div class="divider"></div>

      <!-- ====== 环境音区域 ====== -->
      <div class="panel-section">
        <div class="section-title">
          <Wind :size="16" class="text-cyan-400" />
          <span>环境音</span>
          <button
            v-if="uiStore.ambientTracks.length > 0"
            class="stop-all-btn"
            @click="stopAllAmbient"
          >
            <Square :size="12" /> 全部停止
          </button>
        </div>

        <!-- 活跃轨道列表 -->
        <div v-if="uiStore.ambientTracks.length > 0" class="ambient-track-list">
          <div
            v-for="track in uiStore.ambientTracks"
            :key="track.id"
            class="ambient-track-item"
          >
            <span class="track-name truncate">{{ track.name }}</span>
            <div class="track-controls">
              <button
                class="ctrl-btn small"
                :title="track.paused ? '恢复' : '暂停'"
                @click="uiStore.toggleAmbientTrackPause(track.id)"
              >
                <Play v-if="track.paused" :size="12" />
                <Pause v-else :size="12" />
              </button>
              <input
                type="range"
                min="0"
                max="100"
                :value="track.volume"
                @input="onAmbientVolumeChange(track.id, $event)"
                class="volume-slider mini"
              />
              <button
                class="ctrl-btn small danger"
                title="移除"
                @click="uiStore.removeAmbientTrack(track.id)"
              >
                <X :size="12" />
              </button>
            </div>
          </div>
        </div>

        <!-- 可用环境音文件库 -->
        <div class="ambient-library">
          <div v-if="ambientList.length === 0" class="empty-hint">
            暂无环境音文件，请在设置中上传
          </div>
          <div
            v-for="item in ambientList"
            :key="item.url"
            class="ambient-library-item"
            :class="{ active: isTrackPlaying(item.url) }"
            @click="playAmbientFile(item)"
          >
            <Wind :size="14" />
            <span class="truncate">{{ item.name }}</span>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import {
  Music2, Music, Play, Pause, Square, Repeat, Repeat1, Shuffle, Wind, X, Volume2,
} from 'lucide-vue-next'
import { useUIStore } from '../../../../stores/modules/ui/ui'
import { useSettingsStore } from '../../../../stores/modules/settings'
import { ambientGetAll } from '../../../../api/services/ambient'
import type { AmbientItem } from '../../../../api/services/ambient'

const uiStore = useUIStore()
const settingsStore = useSettingsStore()
const panelVisible = ref(false)
const panelRef = ref<HTMLElement | null>(null)
const ambientList = ref<AmbientItem[]>([])

type PlaybackMode = 'loop-list' | 'loop-single' | 'random'
const modeText: Record<PlaybackMode, string> = {
  'loop-list': '列表循环',
  'loop-single': '单曲循环',
  random: '随机播放',
}

const hasActiveAudio = computed(() =>
  uiStore.ambientTracks.length > 0
  || (uiStore.currentBackgroundMusic !== 'None' && !uiStore.bgMusicPaused),
)

const currentMusicName = computed(() => {
  const url = uiStore.currentBackgroundMusic
  if (!url || url === 'None') return '未选择音乐'
  const fileName = decodeURIComponent(url.split('/').pop() || '')
  return fileName.replace(/\.[^/.]+$/, '') || fileName
})

function togglePanel() {
  panelVisible.value = !panelVisible.value
  if (panelVisible.value) {
    loadAmbientList()
  }
}

function onBgmVolumeChange(e: Event) {
  const val = Number((e.target as HTMLInputElement).value)
  settingsStore.update('audio.backgroundVolume', val)
}

function onAmbientVolumeChange(id: string, e: Event) {
  const val = Number((e.target as HTMLInputElement).value)
  uiStore.updateAmbientTrackVolume(id, val)
}

const togglePlaybackMode = () => {
  const modes: PlaybackMode[] = ['loop-list', 'loop-single', 'random']
  const currentIndex = modes.indexOf(uiStore.bgMusicMode)
  uiStore.bgMusicMode = modes[(currentIndex + 1) % modes.length]
}

const handlePlayPause = () => {
  uiStore.bgMusicPaused = !uiStore.bgMusicPaused
}

const handleStop = () => {
  uiStore.bgMusicStoped = true
  uiStore.bgMusicPaused = true
}

function stopAllAmbient() {
  uiStore.clearAmbientTracks()
}

function isTrackPlaying(src: string): boolean {
  return uiStore.ambientTracks.some((t) => t.src === src && !t.paused)
}

function playAmbientFile(item: AmbientItem) {
  uiStore.addAmbientTrack({
    src: item.url,
    volume: 80,
    loop: true,
    fade: true,
  })
}

async function loadAmbientList() {
  try {
    ambientList.value = await ambientGetAll()
  } catch (e) {
    console.error('加载环境音列表失败:', e)
  }
}

// 点击外部关闭
function handleClickOutside(e: MouseEvent) {
  if (!panelVisible.value) return
  const target = e.target as HTMLElement
  const trigger = document.querySelector('.sound-effect-trigger')
  if (panelRef.value && !panelRef.value.contains(target) && trigger && !trigger.contains(target)) {
    panelVisible.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
})
</script>

<style scoped>
.sound-effect-trigger {
  position: fixed;
  bottom: 1.5rem;
  left: 1.5rem;
  z-index: 5;
  pointer-events: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(18, 18, 28, 0.75);
  backdrop-filter: blur(20px);
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  transition: all 0.3s ease;
}

.sound-effect-trigger:hover {
  background: rgba(255, 255, 255, 0.1);
  color: white;
}

.sound-effect-trigger.has-active {
  animation: pulse-glow 2s ease-in-out infinite;
  border-color: rgba(79, 172, 254, 0.5);
  color: #4facfe;
}

@keyframes pulse-glow {
  0%, 100% { box-shadow: 0 0 0 0 rgba(79, 172, 254, 0.3); }
  50% { box-shadow: 0 0 0 8px rgba(79, 172, 254, 0); }
}

.sound-effect-panel {
  position: fixed;
  bottom: 5rem;
  left: 1.5rem;
  z-index: 5;
  pointer-events: auto;
  width: 520px;
  max-height: 70vh;
  overflow-y: auto;
  background: rgba(18, 18, 28, 0.85);
  backdrop-filter: blur(20px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 1rem;
  padding: 1rem;
  color: white;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}

/* 手机版适配 */
@media (max-width: 767px) {
  .sound-effect-panel {
    left: 50%;
    transform: translateX(-50%);
    width: calc(100% - 2rem);
    max-width: 520px;
    bottom: 4.5rem;
  }

  .sound-effect-trigger {
    bottom: 1rem;
    left: 1rem;
    width: 40px;
    height: 40px;
  }
}

.panel-section {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.section-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.9);
  margin-bottom: 0.25rem;
}

.bgm-current {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: rgba(0, 0, 0, 0.2);
  padding: 0.5rem 0.75rem;
  border-radius: 0.5rem;
  font-size: 0.8125rem;
  color: rgba(255, 255, 255, 0.8);
}

.mode-badge {
  font-size: 0.6875rem;
  color: rgba(255, 255, 255, 0.4);
  background: rgba(255, 255, 255, 0.08);
  padding: 0.125rem 0.5rem;
  border-radius: 999px;
  white-space: nowrap;
}

.control-row {
  display: flex;
  gap: 0.5rem;
}

.ctrl-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 0.5rem;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  transition: all 0.2s;
}

.ctrl-btn:hover {
  background: rgba(255, 255, 255, 0.12);
  color: white;
}

.ctrl-btn.small {
  width: 28px;
  height: 28px;
}

.ctrl-btn.danger:hover {
  background: rgba(239, 68, 68, 0.2);
  color: #ef4444;
}

.volume-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.volume-slider {
  flex: 1;
  height: 4px;
  appearance: none;
  background: rgba(255, 255, 255, 0.15);
  border-radius: 2px;
  outline: none;
  cursor: pointer;
}

.volume-slider::-webkit-slider-thumb {
  appearance: none;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: rgba(79, 172, 254, 0.8);
  border: none;
  cursor: pointer;
}

.volume-slider.mini::-webkit-slider-thumb {
  width: 12px;
  height: 12px;
}

.divider {
  height: 1px;
  background: rgba(255, 255, 255, 0.08);
  margin: 0.75rem 0;
}

.stop-all-btn {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 0.25rem;
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.5);
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 0.375rem;
  padding: 0.25rem 0.5rem;
  cursor: pointer;
  transition: all 0.2s;
}

.stop-all-btn:hover {
  background: rgba(239, 68, 68, 0.15);
  color: #ef4444;
}

.ambient-track-list {
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
}

.ambient-track-item {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  background: rgba(0, 0, 0, 0.2);
  padding: 0.375rem 0.625rem;
  border-radius: 0.5rem;
}

.track-name {
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.7);
}

.track-controls {
  display: flex;
  align-items: center;
  gap: 0.375rem;
}

.ambient-library {
  display: flex;
  flex-wrap: wrap;
  gap: 0.375rem;
  margin-top: 0.25rem;
  max-height: 120px;
  overflow-y: auto;
}

.ambient-library-item {
  display: flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0.25rem 0.625rem;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 0.5rem;
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.7);
  cursor: pointer;
  transition: all 0.2s;
  max-width: 160px;
}

.ambient-library-item:hover {
  background: rgba(255, 255, 255, 0.12);
  color: white;
}

.ambient-library-item.active {
  border-color: rgba(6, 182, 212, 0.5);
  color: #22d3ee;
}

.empty-hint {
  width: 100%;
  text-align: center;
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.3);
  padding: 0.5rem 0;
}
</style>
