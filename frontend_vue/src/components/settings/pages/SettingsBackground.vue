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
                @click="selectBackground(background.url, background.title)"
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
          <div class="text-brand font-bold">
            当前场景：{{ gameStore.currentScene?.sceneName || '无感知' }}
          </div>

          <div class="ml-auto flex gap-4 items-center">
            <button
              class="px-5 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-brand/80 border-brand text-white hover:bg-brand shadow-indigo-500/20"
              @click="openSceneListModal"
            >
              选择场景
            </button>
            <div class="flex items-center gap-2 text-xs text-white/60">
              立马反应
              <Toggle :checked="immediateReaction" @change="immediateReaction = $event" />
            </div>
          </div>
        </div>

        <div class="relative group">
          <div class="absolute -top-3 left-3 px-2 bg-brand/20 backdrop-blur rounded text-[10px] text-brand-light z-10 border border-brand/30">
            SCENE DESCRIPTION
          </div>
          <textarea
            v-model="currentSceneDesc"
            placeholder="输入对当前场景的描述，或者让 AI 视觉识别"
            class="mb-6 w-full px-4 py-4 border rounded-xl text-sm text-white bg-white/5 backdrop-blur-xl border-white/10 shadow-inner focus:outline-none focus:border-brand/50 focus:ring-4 focus:ring-brand/10 transition-all duration-300 min-h-[200px]"
          ></textarea>
        </div>

        <div class="flex w-full gap-6 justify-around items-center">
          <Button type="big" @click="handleSaveScene" :disabled="!currentSceneName">保存场景</Button>
          <Button type="big" @click="handleClearScene" variant="danger">清除场景</Button>
        </div>
      </div>
    </MenuItem>

    <!-- 场景选择弹窗 -->
    <el-dialog v-model="sceneModalVisible" title="选择场景" width="800px" custom-class="scene-modal">
      <div class="grid grid-cols-2 gap-4 max-h-[500px] overflow-y-auto p-2">
        <div
          v-for="scene in scenes"
          :key="scene.sceneName"
          class="scene-item-card group cursor-pointer"
          @click="onSceneSelect(scene)"
        >
          <div class="relative aspect-video rounded-lg overflow-hidden border-2 border-transparent transition-all group-hover:border-brand">
            <img :src="getSceneImageUrl(scene.sceneImage)" class="w-full h-full object-cover" />
            <div class="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent p-3 flex flex-col justify-end">
              <div class="text-white font-bold">{{ scene.sceneName }}</div>
              <div class="text-white/60 text-xs truncate">{{ scene.sceneDescription }}</div>
            </div>
            <div class="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
              <button @click.stop="handleDeleteScene(scene.sceneName)" class="p-1.5 bg-red-500/80 rounded-full hover:bg-red-600">
                <Trash2 :size="14" class="text-white" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </el-dialog>

    <MenuItem title="粒子选择" size="large">
      <template #header>
        <Sparkles :size="20" />
      </template>
      <div class="effect-list">
        <Button type="big" @click="updateParticle(`StarField`)">星空</Button>
        <Button type="big" @click="updateParticle(`Rain`)">雨水</Button>
        <Button type="big" @click="updateParticle(`Sakura`)">樱花</Button>
        <Button type="big" @click="updateParticle(`Snow`)">雪景</Button>
        <Button type="big" @click="updateParticle(`Fireworks`)">烟花</Button>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { Button, Toggle } from '../../base'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { listScenes, saveScene, deleteScene, loadScene, clearScene, type SceneInfo } from '../../../api/services/scene'
import { ElMessage, ElMessageBox } from 'element-plus'
import type { BackgroundImageInfo } from '../../../types'
import {
  getBackgroundImages,
  setCurrentBackground,
  setCurrentBackgroundEffect,
} from '../../../api/services/background'
import { Bubbles, Image, PictureInPicture, Sparkles, Trash2 } from 'lucide-vue-next'

const backgroundList = ref<BackgroundImageInfo[]>([])
const uiStore = useUIStore()
const gameStore = useGameStore()

// 场景相关状态
const scenes = ref<SceneInfo[]>([])
const sceneModalVisible = ref(false)
const immediateReaction = ref(true)
const currentSceneName = ref('')
const currentSceneDesc = ref('')
const currentSceneImage = ref('')

const isSelected = (url: string) => uiStore.currentBackground === url

// 同步当前编辑状态
onMounted(() => {
  if (gameStore.currentScene) {
    currentSceneName.value = gameStore.currentScene.sceneName
    currentSceneDesc.value = gameStore.currentScene.sceneDescription
    currentSceneImage.value = gameStore.currentScene.sceneImage
  }
})

// 获取图片完整 URL
const getSceneImageUrl = (path: string) => {
  if (path.startsWith('http')) return path
  return `/api/v1/chat/background/background_file/${encodeURIComponent(path)}`
}

// 加载场景列表
const fetchScenes = async () => {
  try {
    scenes.value = await listScenes()
  } catch (error) {
    console.error('获取场景列表失败')
  }
}

const openSceneListModal = async () => {
  await fetchScenes()
  sceneModalVisible.value = true
}

// 选择场景并应用
const onSceneSelect = async (scene: SceneInfo) => {
  try {
    await loadScene(scene.sceneName, immediateReaction.value)
    gameStore.setCurrentScene(scene)
    uiStore.currentBackground = getSceneImageUrl(scene.sceneImage)
    currentSceneName.value = scene.sceneName
    currentSceneDesc.value = scene.sceneDescription
    currentSceneImage.value = scene.sceneImage
    sceneModalVisible.value = false
    ElMessage.success(`已切换至场景: ${scene.sceneName}`)
  } catch (error) {
    ElMessage.error('切换场景失败')
  }
}

// 保存/更新场景
const handleSaveScene = async () => {
  if (!currentSceneName.value) {
    ElMessage.warning('请输入场景名称')
    return
  }
  try {
    const sceneData: SceneInfo = {
      sceneName: currentSceneName.value,
      sceneImage: currentSceneImage.value || uiStore.currentBackground.split('/').pop() || '',
      sceneDescription: currentSceneDesc.value
    }
    await saveScene(sceneData)
    ElMessage.success('场景已保存')
    await fetchScenes()
  } catch (error) {
    ElMessage.error('保存失败')
  }
}

// 删除场景
const handleDeleteScene = async (name: string) => {
  try {
    await ElMessageBox.confirm(`确定要删除场景 "${name}" 吗？`, '警告', { type: 'warning' })
    await deleteScene(name)
    await fetchScenes()
    ElMessage.success('已删除')
  } catch (error) {
    // 用户取消或失败
  }
}

// 清除场景
const handleClearScene = async () => {
  try {
    await clearScene()
    gameStore.clearCurrentScene()
    currentSceneName.value = ''
    currentSceneDesc.value = ''
    ElMessage.success('已清除场景感知')
  } catch (error) {
    ElMessage.error('操作失败')
  }
}

// 背景选择逻辑
const selectBackground = async (url: string, title: string) => {
  uiStore.currentBackground = url
  currentSceneImage.value = url.split('/').pop() || ''
  currentSceneName.value = title || currentSceneName.value
  try {
    await setCurrentBackground(url)
  } catch (error) {
    console.error('保存背景失败')
  }
}

// 其他原逻辑保持
const uploadInput = ref<HTMLInputElement | null>(null)
const triggerUpload = () => uploadInput.value?.click()

async function fetchBackgrounds(): Promise<BackgroundImageInfo[]> {
  try {
    const data = await getBackgroundImages()
    return data.map((bg: BackgroundImageInfo) => ({
      title: bg.title || 'Untitled',
      url: `/api/v1/chat/background/background_file/${encodeURIComponent(bg.url)}`,
      time: bg.time,
    }))
  } catch (error) {
    return []
  }
}

const refreshBackground = async () => {
  backgroundList.value = await fetchBackgrounds()
}

const handleFileUpload = async (event: Event) => {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  if (!file) return
  const formData = new FormData()
  formData.append('file', file)
  formData.append('name', file.name)
  try {
    const response = await fetch('/api/v1/chat/background/upload', { method: 'POST', body: formData })
    if (!response.ok) throw new Error()
    await refreshBackground()
    ElMessage.success('上传成功')
  } catch (error) {
    ElMessage.error('上传失败')
  }
}

async function updateParticle(value: string) {
  uiStore.setBackgroundEffect(value)
  try {
    await setCurrentBackgroundEffect(value)
  } catch (error) {
    console.error('保存粒子效果失败')
  }
}

onMounted(refreshBackground)
</script>

<style scoped>
.character-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
  padding-bottom: 20px;
}

.background-card {
  position: relative;
  border-radius: 12px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.background-card:hover {
  transform: translateY(-4px);
  border-color: var(--brand-color);
  box-shadow: 0 12px 24px -12px rgba(var(--brand-color-rgb), 0.5);
}

.background-image {
  width: 100%;
  aspect-ratio: 16/9;
  object-fit: cover;
}

.background-title {
  padding: 10px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(10px);
}

.background-title::before {
  content: attr(data-title);
  color: white;
  font-size: 12px;
  font-weight: 500;
  max-width: 60%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.background-card.selected {
  border: 2px solid var(--brand-color);
}

.scene-item-card {
  transition: transform 0.2s;
}
.scene-item-card:hover {
  transform: scale(1.02);
}

:deep(.scene-modal) {
  background: rgba(20, 20, 20, 0.8) !important;
  backdrop-filter: blur(30px) !important;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 20px;
}
:deep(.el-dialog__title) {
  color: white;
}
</style>
