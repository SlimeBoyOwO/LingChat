<template>
  <MenuPage>
    <MenuItem title="👩 角色音量" size="small">
      <Slider v-model="characterVolume" @change="updateCharacterVolume"> 弱/强 </Slider>
    </MenuItem>

    <MenuItem title="💬 气泡音量" size="small">
      <Slider @change="updateBubbleVolume" v-model="bubbleVolume"> 弱/强 </Slider>
    </MenuItem>

    <MenuItem title="🎶 背景音量" size="small">
      <Slider @change="updateBackgroundVolume" v-model="backgroundVolume"> 弱/强 </Slider>
    </MenuItem>

    <MenuItem title="🔊 声音测试" size="small">
      <div class="sound-test">
        <Button type="big" @click="playCharacterTestSound">测试角色音量</Button>
        <Button type="big" @click="playBubbleTestSound">测试气泡音量</Button>
      </div>
    </MenuItem>

    <MenuItem title="⚙ 背景音乐设置">
      <div class="music-controls">
        <Button type="big" @click="handlePlayPause" class="left-button">{{
          playPauseButtonText
        }}</Button>
        <Button type="big" @click="handleStop" class="left-button">■ 停止</Button>
        <span class="music-name">当前: {{ currentMusicName }}</span>
      </div>

      <div class="music-list-container">
        <div v-if="musicList.length === 0" class="empty-list">暂无音乐，请上传</div>
        <div
          v-for="music in musicList"
          :key="music.url"
          class="music-item"
          @click="playMusic(music)"
        >
          <div class="music-item-name">{{ music.name }}</div>
          <Button @click="deleteMusic(music)" class="action-btn-delete glass-effect">删除</Button>
        </div>
      </div>

      <div class="music-upload">
        <Button type="big" @click="triggerFileUpload">➕ 添加音乐</Button>
        <input
          ref="fileInput"
          type="file"
          @change="handleFileSelect"
          accept=".mp3,.wav,.flac,.webm,.weba,.ogg,.m4a"
          style="display: none"
        />
        <Button type="big" @click="uploadMusic" :disabled="!selectedFile">确认上传</Button>
      </div>
    </MenuItem>

    <audio ref="characterTestPlayer"></audio>
    <audio ref="bubbleTestPlayer"></audio>
    <audio
      ref="backgroundAudioPlayer"
      loop
      @timeupdate="updateMusicState"
      @ended="onMusicEnd"
    ></audio>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { useStorage } from '@vueuse/core'
import { MenuPage, MenuItem } from '../../ui'
import { Slider, Button } from '../../base'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { musicGetAll, musicUpload, musicDelete } from '../../../api/services/music'

// --- 响应式状态和引用 ---

const uiStore = useUIStore()

// 使用 VueUse 的 useStorage 持久化存储音量设置
const characterVolume = useStorage('lingchat-character-volume', 50)
const bubbleVolume = useStorage('lingchat-bubble-volume', 50)
const backgroundVolume = useStorage('lingchat-background-volume', 50)

// 同步 localStorage 中的音量到 Pinia store
watch([characterVolume, bubbleVolume, backgroundVolume], ([charVol, bubVol, bgVol]) => {
  uiStore.characterVolume = charVol
  uiStore.bubbleVolume = bubVol
  uiStore.backgroundVolume = bgVol
}, { immediate: true })

// 音频播放器的模板引用
const characterTestPlayer = ref<HTMLAudioElement | null>(null)
const bubbleTestPlayer = ref<HTMLAudioElement | null>(null)
const backgroundAudioPlayer = ref<HTMLAudioElement | null>(null)

// 背景音乐列表和状态
interface Music {
  name: string
  url: string // 注意：这里的 url 应该是唯一的标识符，比如文件名
}
const musicList = ref<Music[]>([])
const currentMusicName = ref('未选择音乐')
const isMusicPlaying = ref(false)

// 文件上传状态
const selectedFile = ref<File | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)

// --- Pinia Store 音量控制 ---

const updateCharacterVolume = (value: number) => {
  characterVolume.value = value
  if (characterTestPlayer.value) {
    characterTestPlayer.value.volume = value / 100
  }
}

const updateBubbleVolume = (value: number) => {
  bubbleVolume.value = value
  if (bubbleTestPlayer.value) {
    bubbleTestPlayer.value.volume = value / 100
  }
}

const updateBackgroundVolume = (value: number) => {
  backgroundVolume.value = value
  if (backgroundAudioPlayer.value) {
    backgroundAudioPlayer.value.volume = value / 100
  }
}

// 监听 Pinia store 变化，确保音量同步
watch(
  () => uiStore.backgroundVolume,
  (newVolume) => {
    backgroundVolume.value = newVolume
    if (backgroundAudioPlayer.value) {
      backgroundAudioPlayer.value.volume = newVolume / 100
    }
  },
)

// --- 声音测试 ---

const playCharacterTestSound = () => {
  if (!characterTestPlayer.value) return
  // 确保使用正确的资源路径
  characterTestPlayer.value.src = '/audio_effects/角色音量测试.wav'
  characterTestPlayer.value.play().catch((e) => console.error('测试角色音量播放失败:', e))
}

const playBubbleTestSound = () => {
  if (!bubbleTestPlayer.value) return
  bubbleTestPlayer.value.src = '/audio_effects/疑问.wav'
  bubbleTestPlayer.value.play().catch((e) => console.error('测试气泡音量播放失败:', e))
}

// --- 背景音乐 API 交互 ---

const loadMusicList = async () => {
  musicList.value = await musicGetAll()
}

const deleteMusic = async (music: Music) => {
  if (!music) {
    console.log('music对象不存在')
    return
  }
  if (!confirm(`确定要删除《${music.name}》吗？`)) return

  try {
    await musicDelete(music.url)

    console.log(`正在删除音乐: ${music.url}`)
    alert(`《${music.name}》删除成功`)
    await loadMusicList() // 重新加载列表
  } catch (error) {
    console.error('删除音乐失败:', error)
    alert('删除音乐失败')
  }
}

const uploadMusic = async () => {
  if (!selectedFile.value) {
    alert('请先选择一个音乐文件')
    return
  }

  const file = selectedFile.value
  const allowedExts = ['.mp3', '.wav', '.flac', '.webm', '.weba', '.ogg', '.m4a']
  const fileExt = file.name.slice(file.name.lastIndexOf('.')).toLowerCase()

  if (!allowedExts.includes(fileExt)) {
    alert('不支持的音频格式。请上传: ' + allowedExts.join(', '))
    return
  }

  const formData = new FormData()
  formData.append('file', file)

  try {
    await musicUpload(formData)
    console.log(`正在上传文件: ${file.name}`)
    alert('音乐上传成功')

    // 清理并刷新
    selectedFile.value = null
    if (fileInput.value) fileInput.value.value = ''
    await loadMusicList()
  } catch (error) {
    console.error('上传音乐失败:', error)
    alert('音乐上传失败')
  }
}

// --- 背景音乐播放控制 ---

const playPauseButtonText = computed(() => (isMusicPlaying.value ? '⏸ 暂停' : '▶ 播放'))

const playMusic = (music: Music) => {
  if (!backgroundAudioPlayer.value) return
  const musicUrl = `/api/v1/chat/back-music/music_file/${encodeURIComponent(music.url)}`

  backgroundAudioPlayer.value.src = musicUrl
  backgroundAudioPlayer.value.play().catch((e) => console.error('播放音乐失败:', e))
  currentMusicName.value = music.name
  isMusicPlaying.value = true
}

const handlePlayPause = () => {
  if (!backgroundAudioPlayer.value) return

  if (isMusicPlaying.value) {
    backgroundAudioPlayer.value.pause()
  } else {
    // 如果没有 src，播放列表第一首
    if (!backgroundAudioPlayer.value.src && musicList.value.length > 0) {
      playMusic(musicList.value[0] || { name: '', url: '' })
    } else {
      backgroundAudioPlayer.value.play().catch((e) => console.error('恢复播放失败:', e))
    }
  }
  isMusicPlaying.value = !backgroundAudioPlayer.value.paused
}

const handleStop = () => {
  if (!backgroundAudioPlayer.value) return
  backgroundAudioPlayer.value.pause()
  backgroundAudioPlayer.value.currentTime = 0
  isMusicPlaying.value = false
}

// --- 文件上传处理 ---

const triggerFileUpload = () => {
  fileInput.value?.click()
}

const handleFileSelect = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    selectedFile.value = target.files?.[0] || null
  } else {
    selectedFile.value = null
  }
}

// --- Audio 元素事件监听 ---
const updateMusicState = () => {
  if (!backgroundAudioPlayer.value) return
  isMusicPlaying.value = !backgroundAudioPlayer.value.paused
}

const onMusicEnd = () => {
  isMusicPlaying.value = false
}

// --- 生命周期钩子 ---

onMounted(() => {
  // 初始化时加载音乐列表
  loadMusicList()

  // 初始化音量
  if (characterTestPlayer.value) characterTestPlayer.value.volume = characterVolume.value / 100
  if (bubbleTestPlayer.value) bubbleTestPlayer.value.volume = bubbleVolume.value / 100
  if (backgroundAudioPlayer.value)
    backgroundAudioPlayer.value.volume = backgroundVolume.value / 100
})
</script>

<style scoped>
.sound-test,
.music-controls,
.music-upload {
  display: flex;
  justify-content: space-around;
  gap: 20px;
  align-items: center;
}

.music-name {
  flex-grow: 1;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #eee;
}

.music-list-container {
  max-height: 200px;
  overflow-y: auto;
  margin-top: 15px;
  border: 1px solid #555;
  padding: 5px;
  background-color: rgba(0, 0, 0, 0.2);
}

.empty-list {
  text-align: center;
  color: #999;
  padding: 20px;
}

.music-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  cursor: pointer;
  border-bottom: 1px solid #444;
  transition: background-color 0.2s;
}

.music-item:hover {
  background-color: rgba(255, 255, 255, 0.1);
}

.music-item:last-child {
  border-bottom: none;
}

.music-item-name {
  flex-grow: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.action-btn-delete {
  flex-shrink: 0;
  margin-left: 10px;
  /* 你可以为删除按钮定义更小的尺寸 */
  padding: 4px 8px;
  font-size: 12px;
}

.action-btn-delete.glass-effect {
  background: rgba(255, 0, 0, 0.3);
  transition: all 0.2s ease;
}

.action-btn-delete {
  padding: 8px 16px;
  border: 0px solid #555;
  color: #ddd;
  cursor: pointer;
  border-radius: 4px;
  transition:
    background-color 0.2s,
    border-color 0.2s;
  white-space: nowrap;
  font-weight: bold;
}

.action-btn-delete.glass-effect:hover {
  transform: translateY(-1px);
  background: rgba(207, 0, 0, 0.3);
}

.left-button.big {
  width: 20%;
}

.music-upload {
  margin-top: 15px;
}
</style>
