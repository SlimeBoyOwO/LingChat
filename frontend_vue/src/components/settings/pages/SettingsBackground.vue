<template>
  <MenuPage>
    <!-- 新增场景设置区域 -->
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
        <div class="flex gap-3 mb-2">
          <Bubbles />
          <div class="text-brand font-bold">当前场景：无感知</div>

          <div class="ml-auto flex gap-6">
            <button
              class="px-5 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-brand/80 border-brand text-white hover:bg-brand shadow-indigo-500/20"
              @click="updateParticle(`StarField`)"
            >
              选择场景
            </button>
            <Toggle :checked="sceneAwareLocal" @change="onSceneAwareChange">
              切换场景后是否立马回复
            </Toggle>
          </div>
        </div>

        <textarea
          placeholder="输入对当前场景的描述，或者让 AI 视觉识别"
          class="mb-6 w-full px-3 py-2.5 border rounded-lg text-sm text-white bg-white/10 backdrop-blur-xl backdrop-saturate-150 border-white/10 shadow-glass focus:outline-none focus:border-brand focus:ring-2 focus:ring-brand/20 transition-all duration-200"
          rows="8"
        ></textarea>

        <div class="flex w-full gap-6 justify-around items-center">
          <Button type="big" @click="updateParticle(`StarField`)">添加场景</Button>
          <Button type="big" @click="updateParticle(`StarField`)">更新场景</Button>
          <Button type="big" @click="updateParticle(`StarField`)">删除场景</Button>
        </div>
      </div>
    </MenuItem>

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
    <el-dialog v-model="createDialogVisible" title="添加场景" width="400px">
      <el-form label-width="80px">
        <el-form-item label="场景名">
          <el-input v-model="newSceneName" placeholder="例如：海边" />
        </el-form-item>
        <el-form-item label="场景描述">
          <el-input
            v-model="newSceneDescription"
            type="textarea"
            :rows="4"
            placeholder="描述该场景的环境、氛围等"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="createDialogVisible = false">取消</el-button>
          <el-button type="primary" @click="handleCreateScene" :loading="isCreating">
            确定
          </el-button>
        </span>
      </template>
    </el-dialog>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { Button, Toggle } from '../../base' // 确保导入了 Toggle
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { listScenes, loadScene, clearScene, type SceneInfo } from '../../../api/services/scene'
import { ElMessage } from 'element-plus' // 可替换为自定义消息组件
import type { BackgroundImageInfo } from '../../../types'
import { addScene } from '@/api/services/scene'
// 响应式数据
import {
  getBackgroundImages,
  setCurrentBackground,
  setCurrentBackgroundEffect,
} from '../../../api/services/background'
import { Bubbles, Image, PictureInPicture, Sparkle, Sparkles } from 'lucide-vue-next'

const backgroundList = ref<BackgroundImageInfo[]>([])
const selectedBackground = ref<string>('')
const uploadInput = ref<HTMLInputElement | null>(null)
const uiStore = useUIStore()
const gameStore = useGameStore()

// 场景相关状态
const scenes = ref<SceneInfo[]>([])
const isLoadingScenes = ref(false)
const selectedSceneFilename = ref<string>('')
const sceneAwareLocal = ref(gameStore.sceneAware)
// 新增场景对话框
const createDialogVisible = ref(false)
const newSceneName = ref('')
const newSceneDescription = ref('')
const isCreating = ref(false)
// 打开创建对话框
const openCreateDialog = () => {
  newSceneName.value = ''
  newSceneDescription.value = ''
  createDialogVisible.value = true
}
// 提交创建场景
const handleCreateScene = async () => {
  if (!newSceneName.value.trim() || !newSceneDescription.value.trim()) {
    ElMessage.warning('请填写完整')
    return
  }
  isCreating.value = true
  try {
    await addScene(newSceneName.value.trim(), newSceneDescription.value.trim())
    ElMessage.success('场景创建成功')
    createDialogVisible.value = false
    await fetchScenes() // 刷新列表
  } catch (error: any) {
    ElMessage.error(error.message || '创建失败')
  } finally {
    isCreating.value = false
  }
}

// 监听 gameStore.sceneAware 变化
watch(
  () => gameStore.sceneAware,
  (val) => {
    sceneAwareLocal.value = val
  },
)
// 切换场景感知
const onSceneAwareChange = (val: boolean) => {
  gameStore.toggleSceneAware(val)
  if (!val && gameStore.currentScene) {
    handleClearScene() // 关闭感知时自动清除场景
  }
}
// 加载场景列表
const fetchScenes = async () => {
  isLoadingScenes.value = true
  try {
    scenes.value = await listScenes()
  } catch (error) {
    ElMessage.error('获取场景列表失败')
  } finally {
    isLoadingScenes.value = false
  }
}
// 选择场景
const onSceneSelect = async (filename: string) => {
  if (!filename) return
  try {
    await loadScene(filename)
    const scene = scenes.value.find((s) => s.filename === filename)
    if (scene) {
      gameStore.setCurrentScene(scene)

      // 如果存在图片 URL，则更新背景
      if (scene.imageUrl) {
        uiStore.currentBackground = scene.imageUrl
        localStorage.setItem('selectedBackground', scene.imageUrl)
      } else {
        // 纯文本场景且无对应图片，保持当前背景不变
        ElMessage.info('已加载纯文本场景，背景图片保持不变')
      }

      ElMessage.success(`场景“${scene.description}”已加载`)
    }
  } catch (error) {
    ElMessage.error('加载场景失败')
  }
}
// 清除场景
const handleClearScene = async () => {
  try {
    await clearScene()
    gameStore.clearCurrentScene()
    selectedSceneFilename.value = ''
    // 恢复为之前手动选择的背景（从 localStorage 或默认）
    const savedBg = localStorage.getItem('selectedBackground')
    if (savedBg && savedBg !== '') {
      uiStore.currentBackground = savedBg
    } else if (backgroundList.value.length > 0) {
      // 随机选一个背景
      const randomIndex = Math.floor(Math.random() * backgroundList.value.length)
      const randomBg = backgroundList.value[randomIndex]?.url || ''
      uiStore.currentBackground = randomBg
      localStorage.setItem('selectedBackground', randomBg)
    } else {
      uiStore.currentBackground = ''
    }
    ElMessage.success('已清除场景，恢复自由对话模式')
  } catch (error) {
    ElMessage.error('清除场景失败')
  }
}
// 刷新场景列表
const handleRefreshScenes = async () => {
  await fetchScenes()
  // 刷新后检查之前选中的场景是否还存在
  if (
    selectedSceneFilename.value &&
    !scenes.value.some((s) => s.filename === selectedSceneFilename.value)
  ) {
    selectedSceneFilename.value = ''
  }
  ElMessage.success('场景列表已刷新')
}
// 辅助显示
const getSceneDisplayName = (scene: SceneInfo) => {
  return scene.filename.replace(/\.[^/.]+$/, '')
}

onMounted(async () => {
  try {
    await refreshBackground()

    // 检查 uiStore 中是否有已选背景
    if (
      uiStore.currentBackground &&
      uiStore.currentBackground !== '@/assets/images/default_bg.jpg'
    ) {
      selectBackground(uiStore.currentBackground)
    } else if (backgroundList.value.length > 0) {
      // 随机选择一个背景
      const randomIndex = Math.floor(Math.random() * backgroundList.value.length)
      selectBackground(backgroundList.value[randomIndex]?.url || '')
      console.log('已选随机背景')
    }
  } catch (error) {
    console.error('加载背景图片失败', error)
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
  uiStore.currentBackground = url

  try {
    await setCurrentBackground(url)
  } catch (error) {
    selectedBackground.value = prevSelectedBackground
    uiStore.currentBackground = prevBackground
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

onMounted(async () => {
  await refreshBackground()
  await fetchScenes()

  // 恢复之前选择的背景
  const savedBg = localStorage.getItem('selectedBackground')
  if (savedBg) {
    uiStore.currentBackground = savedBg
  } else if (backgroundList.value.length > 0) {
    const randomIndex = Math.floor(Math.random() * backgroundList.value.length)
    uiStore.currentBackground = backgroundList.value[randomIndex]?.url || ''
    localStorage.setItem('selectedBackground', uiStore.currentBackground)
  }

  // 如果 gameStore 中已有当前场景，同步选中项
  if (gameStore.currentScene) {
    selectedSceneFilename.value = gameStore.currentScene.filename
    // 确保背景也是该场景图片
    const sceneImageUrl = `/api/v1/chat/background/background_file/${gameStore.currentScene.filename}`
    uiStore.currentBackground = sceneImageUrl
  }
})
</script>

<style scoped>
/* 确保网格容器正确 */
.backgrounds-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 20px;
  padding-bottom: 20px;
  max-width: 1200px;
  margin: 0 auto;
}

/*什么？你问我为什么这里是character-grid? 灵式编程懂不懂！ */
.character-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 20px;
  padding-bottom: 20px;
  width: 100%;
}

/* 卡片容器 */
/* 为整个卡片添加渐变背景，增强毛玻璃效果 */
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

/* 图片容器 */
/* 图片容器添加伪元素增强效果 */
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
  flex: 1; /* 占据剩余空间 */
  position: relative;
  overflow: hidden;
}

/* 图片样式 */
.background-image {
  position: relative;
  width: 100%;
  height: 100%;
  object-fit: cover;
  aspect-ratio: 16/9; /* 保持图片比例 */
  transition: transform 0.3s ease;
}

/* 底部信息区域 */
/* 修改卡片底部背景为毛玻璃效果 */
.background-title {
  padding: 12px 16px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  background: rgba(255, 255, 255, 0.15); /* 更透明的背景 */
  backdrop-filter: blur(10px); /* 毛玻璃效果 */
  -webkit-backdrop-filter: blur(10px); /* Safari 支持 */
  border-top: 1px solid rgba(255, 255, 255, 0.2); /* 更柔和的边框 */
  position: relative;
  z-index: 2;
}
/* 标题文本样式 */
/* 标题文字颜色调整以适应毛玻璃背景 */
.background-title::before {
  content: attr(data-title);
  font-weight: 500;
  color: rgba(255, 255, 255, 0.9); /* 更亮的文字颜色 */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 70%;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

/* 选择按钮 */
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

/* 交互效果 */
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

.effect-list {
  display: flex;
  justify-content: space-around;
  gap: 20px;
  align-items: center;
}

/* 响应式调整 */
@media (max-width: 768px) {
  #backgrounds-list {
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 16px;
    padding: 12px;
  }
}

@media (max-width: 480px) {
  #backgrounds-list {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 768px) {
  /*什么？你问我为什么这里是character-grid? 灵式编程懂不懂！ */
  .character-grid {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  }
}

/* 选中状态的卡片样式 */
.background-card.selected {
  border: 2px solid #3bc7f6d8;
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.3);
}

/* 已选中按钮样式 */
.background-select-btn.selected {
  background-color: #10b981 !important;
}
</style>
