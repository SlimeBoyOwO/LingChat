<template>
  <MenuPage>
    <MenuItem title="背景选择">
      <template #header>
        <Image :size="20" />
      </template>
      <div class="background-container">
        <div class="background-list character-grid">
          <div
            v-for="(background, index) in backgroundList"
            :key="index"
            :class="['background-card', { selected: isSelected(background.url) }]"
          >
            <div class="background-image-container">
              <img :src="background.url" :alt="background.title" class="background-image" />
            </div>
            <div class="background-title" :data-title="background.title">
              <Button
                :class="['background-select-btn', { selected: isSelected(background.url) }]"
                @click="selectBackground(background.url)"
              >
                {{ isSelected(background.url) ? '已选中' : '选择' }}
              </Button>
            </div>
          </div>
        </div>

        <Button type="big" @click="triggerUpload">上传自定义背景</Button>
        <input
          type="file"
          ref="uploadInput"
          @change="handleFileUpload"
          accept=".jpg,.png,.webp,.bmp,.svg,.tif,.gif"
          style="display: none"
        />
      </div>
    </MenuItem>

    <MenuItem title="场景感知">
      <template #header>
        <PictureInPicture :size="20" />
      </template>
      <div class="p-2 flex flex-col gap-2 justify-center">
        <div class="flex gap-3 mb-2 items-center">
          <Bubbles />
          <div class="text-brand font-bold">当前场景：{{ currentSceneDisplay }}</div>

          <div class="ml-auto flex gap-6">
            <button
              class="px-5 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-brand/80 border-brand text-white hover:bg-brand shadow-indigo-500/20"
              @click="showSceneSelect = true"
            >
              选择场景
            </button>
          </div>
        </div>

        <div class="flex w-full gap-6 justify-around items-center">
          <Button type="big" @click="handleCreateScene">添加场景</Button>
          <Button type="big" @click="handleUpdateScene" :disabled="!currentScene">更新场景</Button>
          <Button type="big" @click="handleDeleteScene" :disabled="!currentScene">删除场景</Button>
        </div>
      </div>
    </MenuItem>

    <MenuItem title="粒子选择" size="large">
      <template #header>
        <Sparkles :size="20" />
      </template>
      <div class="effect-list flex gap-4 overflow-x-auto pb-2">
        <Button type="big" @click="updateParticle(`None`)">无</Button>
        <Button type="big" @click="updateParticle(`StarField`)">星空</Button>
        <Button type="big" @click="updateParticle(`Rain`)">雨水</Button>
        <Button type="big" @click="updateParticle(`Sakura`)">樱花</Button>
        <Button type="big" @click="updateParticle(`Snow`)">雪景</Button>
        <Button type="big" @click="updateParticle(`Fireworks`)">烟花</Button>
      </div>
    </MenuItem>

    <MenuItem title="动画开关" size="large">
      <template #header>
        <Settings :size="20" />
      </template>
      <div class="flex flex-col gap-3">
        <Toggle
          :checked="mainMenuStarsEnabled"
          @change="settingsStore.setMainMenuStarsEnabled($event)"
        >
          启用主界面星星粒子
        </Toggle>
        <Toggle
          :checked="mainMenuMeteorsEnabled"
          @change="settingsStore.setMainMenuMeteorsEnabled($event)"
        >
          启用主界面流星动画
        </Toggle>
        <Toggle
          :checked="globalMouseTrailEnabled"
          @change="settingsStore.setGlobalMouseTrailEnabled($event)"
        >
          启用全局鼠标滑动动画
        </Toggle>
        <Toggle
          :checked="clickAnimationEnabled"
          @change="settingsStore.setClickAnimationEnabled($event)"
        >
          启用点击动画
        </Toggle>
      </div>
    </MenuItem>

    <MenuItem title="动画设置" size="large">
      <template #header>
        <Sparkles :size="20" />
      </template>
      <div class="flex flex-col gap-4 p-2">
        <!-- 流星帧率 -->
        <div class="flex items-center gap-4">
          <span class="text-sm font-medium text-white/90 min-w-[120px]">流星帧率 (FPS)</span>
          <Slider
            v-model="meteorFps"
            :min="10"
            :max="60"
            :step="5"
            accent-color="#8b5cf6"
            @change="handleMeteorFpsChange"
            class="flex-1"
          >
            <template #left>{{ meteorFps }} FPS</template>
          </Slider>
          <input
            type="number"
            v-model.number="meteorFpsInput"
            @blur="handleInputBlur"
            @keyup.enter="handleInputEnter"
            min="10"
            max="300"
            class="w-20 px-3 py-1.5 rounded-lg bg-white/10 border border-white/20 text-white text-sm font-medium focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition-all"
          />
        </div>

        <!-- 星星帧率 -->
        <div class="flex items-center gap-4">
          <span class="text-sm font-medium text-white/90 min-w-[120px]">星星帧率 (FPS)</span>
          <Slider
            v-model="starsFps"
            :min="10"
            :max="60"
            :step="5"
            accent-color="#fbbf24"
            @change="handleStarsFpsChange"
            class="flex-1"
          >
            <template #left>{{ starsFps }} FPS</template>
          </Slider>
          <input
            type="number"
            v-model.number="starsFpsInput"
            @blur="handleStarsInputBlur"
            @keyup.enter="handleStarsInputEnter"
            min="10"
            max="300"
            class="w-20 px-3 py-1.5 rounded-lg bg-white/10 border border-white/20 text-white text-sm font-medium focus:outline-none focus:ring-2 focus:ring-yellow-500 focus:border-transparent transition-all"
          />
        </div>
      </div>
    </MenuItem>

    <SceneSelectModal
      :show="showSceneSelect"
      :scenes="scenes"
      @close="showSceneSelect = false"
      @confirm="handleSceneSelect"
    />

    <SceneEditModal
      :show="showSceneEdit"
      :mode="editMode"
      :backgrounds="backgroundList"
      :initial-data="editInitialData"
      @close="showSceneEdit = false"
      @submit="handleSceneSubmit"
      @upload="triggerUpload"
    />
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, watch, computed } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { Button, Toggle, Slider } from '../../base'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useSettingsStore } from '../../../stores/modules/settings'
import {
  listScenes,
  loadScene,
  createScene,
  updateScene,
  deleteScene,
  type SceneInfo,
} from '../../../api/services/scene'
import type { BackgroundImageInfo } from '../../../types'
import {
  getBackgroundImages,
  setCurrentBackground,
  setCurrentBackgroundEffect,
} from '../../../api/services/background'
import { Bubbles, Image, PictureInPicture, Sparkles, Settings } from 'lucide-vue-next'
import SceneSelectModal from '../scene/SceneSelectModal.vue'
import SceneEditModal from '../scene/SceneEditModal.vue'

const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()

const mainMenuStarsEnabled = computed(() => settingsStore.mainMenuStarsEnabled)
const mainMenuMeteorsEnabled = computed(() => settingsStore.mainMenuMeteorsEnabled)
const globalMouseTrailEnabled = computed(() => settingsStore.globalMouseTrailEnabled)
const clickAnimationEnabled = computed(() => settingsStore.clickAnimationEnabled)
const meteorFps = computed({
  get: () => settingsStore.meteorFps,
  set: (value: number) => {
    const clampedValue = Math.max(10, Math.min(60, value))
    settingsStore.setMeteorFps(clampedValue)
  },
})
const meteorFpsInput = ref(settingsStore.meteorFps)

const starsFps = computed({
  get: () => settingsStore.starsFps,
  set: (value: number) => {
    const clampedValue = Math.max(10, Math.min(60, value))
    settingsStore.setStarsFps(clampedValue)
  },
})
const starsFpsInput = ref(settingsStore.starsFps)

const backgroundList = ref<BackgroundImageInfo[]>([])
const selectedBackground = ref<string>('')
const uploadInput = ref<HTMLInputElement | null>(null)

const scenes = ref<SceneInfo[]>([])
const sceneAwareLocal = ref(gameStore.sceneAware)

const showSceneSelect = ref(false)
const showSceneEdit = ref(false)
const editMode = ref<'create' | 'update'>('create')
const editInitialData = ref<
  { sceneName: string; sceneImage: string | null; sceneDescription: string } | undefined
>()

const currentSceneDisplay = computed(() => gameStore.currentScene?.sceneName || '无感知')
const currentScene = computed(() => gameStore.currentScene)

watch(
  () => gameStore.sceneAware,
  (val) => {
    sceneAwareLocal.value = val
  },
)

const fetchScenes = async () => {
  try {
    scenes.value = await listScenes()
  } catch (error) {
    console.error('获取场景列表失败', error)
  }
}

const handleSceneSelect = async (sceneId: string) => {
  try {
    const scene = scenes.value.find((s) => s.id === sceneId)
    if (!scene) return

    await loadScene(sceneId, sceneAwareLocal.value)
    gameStore.setCurrentScene(scene)

    if (scene.imageUrl) {
      uiStore.setCurrentBackground(scene.imageUrl)
    }

    showSceneSelect.value = false
  } catch (error) {
    console.error('加载场景失败', error)
  }
}

const handleCreateScene = () => {
  editMode.value = 'create'
  editInitialData.value = undefined
  showSceneEdit.value = true
}

const handleUpdateScene = () => {
  if (!currentScene.value) return

  editMode.value = 'update'
  editInitialData.value = {
    sceneName: currentScene.value.sceneName,
    sceneImage: currentScene.value.sceneImage || null,
    sceneDescription: currentScene.value.sceneDescription,
  }
  showSceneEdit.value = true
}

const handleDeleteScene = async () => {
  if (!currentScene.value) return
  if (!confirm(`确定要删除场景"${currentScene.value.sceneName}"吗？`)) return

  try {
    await deleteScene(currentScene.value.id)
    gameStore.clearCurrentScene()
    await fetchScenes()
  } catch (error) {
    console.error('删除场景失败', error)
  }
}

const handleSceneSubmit = async (data: {
  sceneName: string
  sceneImage: string | null
  sceneDescription: string
  autoAnalyze: boolean
}) => {
  try {
    if (editMode.value === 'create') {
      await createScene(data)
      await fetchScenes()
    } else {
      if (!currentScene.value) return
      const updated = await updateScene(currentScene.value.id, data)
      gameStore.setCurrentScene(updated)
      await fetchScenes()
    }
    showSceneEdit.value = false
  } catch (error) {
    console.error('操作失败', error)
  }
}

const onSceneAwareChange = (val: boolean) => {
  gameStore.toggleSceneAware(val)
}

onMounted(async () => {
  try {
    await refreshBackground()

    if (
      uiStore.currentBackground &&
      uiStore.currentBackground !== '@/assets/images/default_bg.jpg'
    ) {
      selectBackground(uiStore.currentBackground)
    } else if (backgroundList.value.length > 0) {
      const randomIndex = Math.floor(Math.random() * backgroundList.value.length)
      selectBackground(backgroundList.value[randomIndex]?.url || '')
    }
  } catch (error) {
    console.error('加载背景图片失败', error)
  }

  await fetchScenes()

  if (gameStore.currentScene && gameStore.currentScene.imageUrl) {
    uiStore.setCurrentBackground(gameStore.currentScene.imageUrl)
  }
})

async function fetchBackgrounds(): Promise<BackgroundImageInfo[]> {
  try {
    const data = await getBackgroundImages()
    return data.map((background: BackgroundImageInfo) => ({
      title: background.title || 'Untitled',
      url: background.url
        ? `/api/v1/chat/background/background_file/${encodeURIComponent(background.url)}`
        : '../pictures/background/default.png',
      time: background.time,
    }))
  } catch (error) {
    console.error('Failed to fetch background list:', error)
    return []
  }
}

async function refreshBackground(): Promise<void> {
  backgroundList.value = await fetchBackgrounds()
}

function isSelected(url: string): boolean {
  return selectedBackground.value === url
}

async function selectBackground(url: string): Promise<void> {
  const prevSelectedBackground = selectedBackground.value
  const prevBackground = uiStore.currentBackground

  selectedBackground.value = url
  uiStore.setCurrentBackground(url)

  try {
    await setCurrentBackground(url)
  } catch (error) {
    selectedBackground.value = prevSelectedBackground
    uiStore.setCurrentBackground(prevBackground)
    console.error('Failed to save selected background:', error)
  }
}

function triggerUpload(): void {
  uploadInput.value?.click()
}

async function handleFileUpload(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return

  const fileName = file.name
  const fileExt = fileName.slice(fileName.lastIndexOf('.')).toLowerCase()

  const allowedExts = ['.jpg', '.png', '.webp', '.bmp', '.svg', '.tif', '.gif']

  if (!allowedExts.includes(fileExt)) {
    alert('请上传支持的图片格式: ' + allowedExts.join(', '))
    return
  }

  const formData = new FormData()
  formData.append('file', file)
  formData.append('name', fileName)

  try {
    const response = await fetch('/api/v1/chat/background/upload', {
      method: 'POST',
      body: formData,
    })

    if (!response.ok) throw new Error('上传失败')

    await refreshBackground()

    if (target) target.value = ''
  } catch (error) {
    console.error('上传失败', error)
    alert('上传失败，请重试')
  }
}

async function updateParticle(value: string): Promise<void> {
  const prevEffect = uiStore.currentBackgroundEffect
  uiStore.setBackgroundEffect(value)

  try {
    await setCurrentBackgroundEffect(value)
  } catch (error) {
    uiStore.setBackgroundEffect(prevEffect)
    console.error('Failed to save selected background effect:', error)
  }
}

// 处理滑块变化
function handleMeteorFpsChange(value: number) {
  const clampedValue = Math.max(10, Math.min(60, value))
  meteorFpsInput.value = clampedValue
  settingsStore.setMeteorFps(clampedValue)
}

// 处理输入框失去焦点
function handleInputBlur() {
  let value = Number(meteorFpsInput.value)
  if (isNaN(value) || value < 10) {
    value = 10
  } else if (value > 300) {
    value = 300
  }
  meteorFpsInput.value = value
  settingsStore.setMeteorFps(value)
}

// 处理输入框回车
function handleInputEnter() {
  handleInputBlur()
}

// 监听meteorFps变化，同步更新输入框
watch(meteorFps, (newValue) => {
  meteorFpsInput.value = newValue
})

// 处理星星滑块变化
function handleStarsFpsChange(value: number) {
  const clampedValue = Math.max(10, Math.min(60, value))
  starsFpsInput.value = clampedValue
  settingsStore.setStarsFps(clampedValue)
}

// 处理星星输入框失去焦点
function handleStarsInputBlur() {
  let value = Number(starsFpsInput.value)
  if (isNaN(value) || value < 10) {
    value = 10
  } else if (value > 300) {
    value = 300
  }
  starsFpsInput.value = value
  settingsStore.setStarsFps(value)
}

// 处理星星输入框回车
function handleStarsInputEnter() {
  handleStarsInputBlur()
}

// 监听starsFps变化，同步更新输入框
watch(starsFps, (newValue) => {
  starsFpsInput.value = newValue
})
</script>

<style scoped>
.backgrounds-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 20px;
  padding-bottom: 20px;
  max-width: 1200px;
  margin: 0 auto;
}

.character-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 20px;
  padding-bottom: 20px;
  width: 100%;
}

.background-card {
  position: relative;
  display: flex;
  flex-direction: column;
  border-radius: 12px;
  overflow: hidden;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  transition: all 0.3s ease;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.125);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
}

.background-image-container::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(to bottom, transparent 60%, rgba(0, 0, 0, 0.3) 100%);
  z-index: 1;
  pointer-events: none;
}

.background-image-container {
  flex: 1;
  position: relative;
  overflow: hidden;
}

.background-image {
  position: relative;
  width: 100%;
  height: 100%;
  object-fit: cover;
  aspect-ratio: 16/9;
  transition: transform 0.3s ease;
}

.background-title {
  padding: 12px 16px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: rgba(255, 255, 255, 0.15);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  border-top: 1px solid rgba(255, 255, 255, 0.2);
  position: relative;
  z-index: 2;
}

.background-title::before {
  content: attr(data-title);
  font-weight: 500;
  color: rgba(255, 255, 255, 0.9);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 70%;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.background-select-btn {
  padding: 6px 12px;
  background: #4f46e5;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
  flex-shrink: 0;
}

.background-card:hover {
  background: rgba(255, 255, 255, 0.15);
  backdrop-filter: blur(25px) saturate(200%);
  transform: translateY(-4px) scale(1.01);
  box-shadow:
    0 12px 40px rgba(0, 0, 0, 0.15),
    inset 0 2px 2px rgba(255, 255, 255, 0.15);
}

.background-card:hover .background-image {
  transform: scale(1.03);
}

.background-select-btn:hover {
  background: #4338ca;
  transform: translateY(-1px);
}

.background-select-btn:active {
  transform: translateY(0);
}

@media (max-width: 768px) {
  .character-grid {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  }
}

.background-card.selected {
  border: 2px solid #3bc7f6d8;
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.3);
}

.background-select-btn.selected {
  background-color: #10b981 !important;
}
</style>
