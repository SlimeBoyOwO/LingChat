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

    <MenuItem title="🏆 成就音量" size="small">
      <Slider @change="updateAchievementVolume" v-model="achievementVolume"> 弱/强 </Slider>
    </MenuItem>

    <MenuItem title="🔊 声音测试" size="small">
      <div class="sound-test">
        <Button type="big" @click="playCharacterTestSound">测试角色音量</Button>
        <Button type="big" @click="playBubbleTestSound">测试气泡音量</Button>
        <Button type="big" @click="playAchievementTestSound">测试成就音量</Button>
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
          <Button @click.stop="deleteMusic(music)" class="action-btn-delete glass-effect">
            删除
          </Button>
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
    <audio ref="achievementTestPlayer"></audio>
    <audio
      ref="backgroundAudioPlayer"
      loop
      @timeupdate="updateMusicState"
      @ended="onMusicEnd"
    ></audio>
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

const uiStore = useUIStore()
const settingsStore = useSettingsStore()

// 使用 computed 绑定 settings store 的音量值
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

const characterTestPlayer = ref<HTMLAudioElement | null>(null)
const bubbleTestPlayer = ref<HTMLAudioElement | null>(null)
const achievementTestPlayer = ref<HTMLAudioElement | null>(null)
const backgroundAudioPlayer = ref<HTMLAudioElement | null>(null)

interface Music {
  name: string
  url: string
}

const musicList = ref<Music[]>([])
const currentMusicName = ref('未选择音乐')
const isMusicPlaying = ref(false)

const selectedFile = ref<File | null>(null)
const fileInput = ref<HTMLInputElement | null>(null)

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

const updateCharacterVolume = (value: number) => {
  settingsStore.update('audio.characterVolume', value)
  if (characterTestPlayer.value) {
    characterTestPlayer.value.volume = value / 100
  }
}

const updateBubbleVolume = (value: number) => {
  settingsStore.update('audio.bubbleVolume', value)
  if (bubbleTestPlayer.value) {
    bubbleTestPlayer.value.volume = value / 100
  }
}

const updateBackgroundVolume = (value: number) => {
  settingsStore.update('audio.backgroundVolume', value)
  if (backgroundAudioPlayer.value) {
    backgroundAudioPlayer.value.volume = value / 100
  }
}

const updateAchievementVolume = (value: number) => {
  settingsStore.update('audio.achievementVolume', value)
  if (achievementTestPlayer.value) {
    achievementTestPlayer.value.volume = value / 100
  }
}

watch(
  () => settingsStore.backgroundVolume,
  (newVolume) => {
    if (backgroundAudioPlayer.value) {
      backgroundAudioPlayer.value.volume = newVolume / 100
    }
  },
)

watch(
  () => uiStore.currentBackgroundMusic,
  () => {
    syncCurrentMusicName()
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

const deleteMusic = async (music: Music) => {
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
      isMusicPlaying.value = false
    }

    await loadMusicList()
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
    selectedFile.value = null
    if (fileInput.value) fileInput.value.value = ''
    await loadMusicList()
    alert('音乐上传成功')
  } catch (error) {
    console.error('上传音乐失败:', error)
    alert('音乐上传失败')
  }
}

const playPauseButtonText = computed(() => (isMusicPlaying.value ? '⏸ 暂停' : '▶ 播放'))

const playMusic = async (music: Music) => {
  if (!backgroundAudioPlayer.value) return

  const musicUrl = toMusicUrl(music.url)
  backgroundAudioPlayer.value.src = musicUrl
  backgroundAudioPlayer.value.play().catch((e) => console.error('播放音乐失败:', e))

  currentMusicName.value = music.name
  isMusicPlaying.value = true
  uiStore.currentBackgroundMusic = musicUrl

  try {
    await setCurrentBackgroundMusic(musicUrl)
  } catch (error) {
    console.error('保存背景音乐失败:', error)
  }
}

const handlePlayPause = () => {
  if (!backgroundAudioPlayer.value) return

  if (isMusicPlaying.value) {
    backgroundAudioPlayer.value.pause()
  } else if (!backgroundAudioPlayer.value.src && musicList.value.length > 0) {
    void playMusic(musicList.value[0] || { name: '', url: '' })
  } else {
    backgroundAudioPlayer.value.play().catch((e) => console.error('恢复播放失败:', e))
  }

  isMusicPlaying.value = !backgroundAudioPlayer.value.paused
}

const handleStop = () => {
  if (!backgroundAudioPlayer.value) return
  backgroundAudioPlayer.value.pause()
  backgroundAudioPlayer.value.currentTime = 0
  isMusicPlaying.value = false
}

const triggerFileUpload = () => {
  fileInput.value?.click()
}

const handleFileSelect = (event: Event) => {
  const target = event.target as HTMLInputElement
  if (target.files && target.files.length > 0) {
    selectedFile.value = target.files[0] || null
  } else {
    selectedFile.value = null
  }
}

const updateMusicState = () => {
  if (!backgroundAudioPlayer.value) return
  isMusicPlaying.value = !backgroundAudioPlayer.value.paused
}

const onMusicEnd = () => {
  isMusicPlaying.value = false
}

onMounted(async () => {
  await loadMusicList()

  if (characterTestPlayer.value) characterTestPlayer.value.volume = characterVolume.value / 100
  if (bubbleTestPlayer.value) bubbleTestPlayer.value.volume = bubbleVolume.value / 100
  if (achievementTestPlayer.value) {
    achievementTestPlayer.value.volume = achievementVolume.value / 100
  }
  if (backgroundAudioPlayer.value) {
    backgroundAudioPlayer.value.volume = backgroundVolume.value / 100

    if (uiStore.currentBackgroundMusic && uiStore.currentBackgroundMusic !== 'None') {
      backgroundAudioPlayer.value.src = uiStore.currentBackgroundMusic
    }
  }

  syncCurrentMusicName()
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
