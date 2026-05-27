<template>
  <MenuPage>
    <!-- ========== 场景管理 ========== -->
    <MenuItem title="场景管理">
      <template #header>
        <PictureInPicture :size="20" />
      </template>

      <!-- 当前场景信息 + 操作按钮 -->
      <div class="flex items-center gap-3 mb-4">
        <div class="text-brand font-bold">当前场景：{{ currentSceneDisplay }}</div>
        <div class="ml-auto flex gap-3">
          <button
            class="px-5 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-brand/80 border-brand text-white hover:bg-brand shadow-indigo-500/20"
            @click="handleCreateScene"
          >
            创建场景
          </button>
          <button
            class="px-4 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-white/10 border-white/20 text-white/80 hover:bg-white/20"
            @click="triggerUpload"
          >
            上传背景
          </button>
          <button
            class="px-4 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-white/10 border-white/20 text-white/80 hover:bg-white/20"
            @click="handleOpenFolder"
          >
            打开文件夹
          </button>
          <button
            class="px-4 py-1.5 rounded-full text-sm font-bold transition-all border shadow-lg bg-red-500/20 border-red-500/30 text-red-300 hover:bg-red-500/30"
            :disabled="!currentScene"
            @click="handleDeleteScene"
          >
            删除
          </button>
        </div>
      </div>

      <!-- 场景卡片网格 -->
      <div class="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-5 pb-5 w-full">
        <div
          v-for="scene in paginatedScenes"
          :key="scene.id"
          :class="[
            'relative flex flex-col rounded-xl overflow-hidden bg-white/10 backdrop-blur-[20px] backdrop-saturate-180 border border-white/12.5 shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)] transition-all duration-300 hover:bg-white/15 hover:backdrop-blur-[25px] hover:backdrop-saturate-200 hover:-translate-y-1 hover:scale-[1.01] hover:shadow-[0_12px_40px_rgba(0,0,0,0.15),inset_0_2px_2px_rgba(255,255,255,0.15)] cursor-pointer group',
            isSceneSelected(scene.id)
              ? '!border-2 !border-sky-400 shadow-[0_0_12px_rgba(56,189,248,0.5),0_0_3px_rgba(56,189,248,0.8),inset_0_0_8px_rgba(56,189,248,0.15)]'
              : '',
          ]"
          @click="handleSceneClick(scene)"
        >
          <!-- 编辑按钮（右上角扳手） -->
          <button
            class="absolute top-2 right-2 z-10 p-1.5 rounded-lg bg-black/50 text-white/60 hover:text-white hover:bg-black/70 transition-all opacity-0 group-hover:opacity-100"
            @click.stop="handleWrenchClick(scene)"
            title="编辑场景"
          >
            <Wrench :size="16" />
          </button>

          <!-- 背景预览 -->
          <div
            class="flex-1 relative overflow-hidden after:absolute after:inset-0 after:bg-linear-to-b after:from-transparent after:to-black/30 after:pointer-events-none"
          >
            <img
              v-if="scene.background"
              :src="convertFileSrc(scene.background)"
              :alt="scene.scene_name"
              class="w-full h-full object-cover aspect-video transition-transform duration-300 group-hover:scale-[1.03]"
            />
            <div v-else class="w-full h-full aspect-video bg-black/40 flex items-center justify-center text-white/20">
              <Image :size="48" />
            </div>
          </div>

          <!-- 信息栏 -->
          <div
            class="px-4 py-3 flex flex-col gap-1 bg-white/15 backdrop-blur-[10px] border-t border-white/20 relative z-2"
          >
            <span class="font-medium text-white/90 truncate drop-shadow-md">
              {{ scene.scene_name }}
            </span>
            <span
              v-if="scene.scene_description"
              class="text-xs text-white/50 line-clamp-2"
            >{{ scene.scene_description }}</span>
            <span
              v-else
              class="text-xs text-yellow-400/60 italic"
            >暂无描述（选择后不会触发旁白）</span>
          </div>
        </div>
      </div>

      <!-- 分页控件 -->
      <div
        v-if="totalPages > 1"
        class="flex items-center justify-center gap-2 pb-2"
      >
        <button
          :disabled="currentPage <= 1"
          @click="currentPage = 1"
          class="px-2 py-1 text-xs text-white/50 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        >
          ← 首页
        </button>
        <button
          :disabled="currentPage <= 1"
          @click="currentPage = currentPage - 1"
          class="px-3 py-1 text-sm text-white/50 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        >
          上一页
        </button>
        <span class="text-xs text-white/60 px-3">
          第 {{ currentPage }} / {{ totalPages }} 页
        </span>
        <button
          :disabled="currentPage >= totalPages"
          @click="currentPage = currentPage + 1"
          class="px-3 py-1 text-sm text-white/50 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        >
          下一页
        </button>
        <button
          :disabled="currentPage >= totalPages"
          @click="currentPage = totalPages"
          class="px-2 py-1 text-xs text-white/50 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        >
          末页 →
        </button>
      </div>

      <!-- 隐藏的文件上传 input -->
      <input
        type="file"
        ref="uploadInput"
        @change="handleFileUpload"
        accept=".jpg,.jpeg,.png,.webp,.bmp,.svg,.tif,.gif"
        style="display: none"
      />
    </MenuItem>

    <MenuItem title="AI 生成背景">
      <template #header>
        <Wand2 :size="20" />
      </template>
      <div class="flex flex-col gap-3 p-2">
        <div class="text-xs text-white/60">
          输入描述文字，AI 将为你自动生成一张背景图片并添加到图库中
        </div>
        <div class="flex flex-col gap-2 items-center">
          <input
            v-model="generatePrompt"
            type="text"
            placeholder="描述你想要的背景，例如：夜晚的樱花小路，二次元风格..."
            class="w-full flex-1 px-3 py-2 rounded-lg bg-white/10 border border-white/20 text-white text-sm placeholder-white/40 focus:outline-none focus:ring-2 focus:ring-purple-500 focus:border-transparent transition-all"
            :disabled="isGenerating"
            @keyup.enter="handleGenerate"
          />
          <Button
            type="big"
            :disabled="isGenerating || !generatePrompt.trim()"
            @click="handleGenerate"
          >
            {{ isGenerating ? '生成中...' : '生成' }}
          </Button>
        </div>
        <p v-if="isGenerating" class="text-xs text-white/50">
          正在后台生成中，完成后会自动通知你...
        </p>
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
        <Toggle
          :checked="sceneAwarenessEnabled"
          @change="settingsStore.setSceneAwarenessEnabled($event)"
        >
          启用场景感知（切换场景时触发旁白）
        </Toggle>
      </div>
    </MenuItem>

    <MenuItem title="动画设置" size="large">
      <template #header>
        <Sparkles :size="20" />
      </template>
      <div class="flex flex-col gap-4 p-2">
        <div class="flex items-center gap-4">
          <span class="text-sm font-medium text-white/90 min-w-30">流星帧率 (FPS)</span>
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

        <div class="flex items-center gap-4">
          <span class="text-sm font-medium text-white/90 min-w-30">星星帧率 (FPS)</span>
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
import { ref, onMounted, onUnmounted, watch, computed } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { MenuPage, MenuItem } from '../../ui'
import { Button, Toggle, Slider } from '../../base'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useDialogStore } from '../../../stores/modules/ui/dialog'
import { useSettingsStore } from '../../../stores/modules/settings'
import {
  listScenes,
  createScene,
  updateScene,
  deleteScene,
  selectScene,
  type SceneInfo,
  type LightingParams,
} from '../../../api/services/scene'
import type { BackgroundImageInfo } from '../../../types'
import {
  getBackgroundImages,
  uploadBackgroundImage,
  generateBackgroundImage,
  openBackgroundsFolder,
} from '../../../api/services/background'
import { Image, PictureInPicture, Sparkles, Settings, Wand2, Wrench } from 'lucide-vue-next'
import SceneEditModal from '../scene/SceneEditModal.vue'
import { useUserStore } from '../../../stores/modules/user/user'

const gameStore = useGameStore()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()
const userStore = useUserStore()
const dialogStore = useDialogStore()

const mainMenuStarsEnabled = computed(() => settingsStore.mainMenuStarsEnabled)
const mainMenuMeteorsEnabled = computed(() => settingsStore.mainMenuMeteorsEnabled)
const globalMouseTrailEnabled = computed(() => settingsStore.globalMouseTrailEnabled)
const clickAnimationEnabled = computed(() => settingsStore.clickAnimationEnabled)
const sceneAwarenessEnabled = computed(() => settingsStore.sceneAwarenessEnabled)
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
const uploadInput = ref<HTMLInputElement | null>(null)
const generatePrompt = ref('')
const isGenerating = ref(false)

const scenes = ref<SceneInfo[]>([])

// 分页
const ITEMS_PER_PAGE = 6
const currentPage = ref(1)
const totalPages = computed(() => Math.max(1, Math.ceil(scenes.value.length / ITEMS_PER_PAGE)))
const paginatedScenes = computed(() => {
  const start = (currentPage.value - 1) * ITEMS_PER_PAGE
  return scenes.value.slice(start, start + ITEMS_PER_PAGE)
})
// 场景变化时回到第一页
watch(scenes, () => {
  if (currentPage.value > totalPages.value) currentPage.value = totalPages.value
})

const showSceneEdit = ref(false)
const editMode = ref<'create' | 'update'>('create')
const editingSceneId = ref<string | null>(null)
const editInitialData = ref<
  { sceneName: string; sceneImage: string | null; sceneDescription: string; lighting?: LightingParams | null } | undefined
>()

const currentSceneDisplay = computed(() => gameStore.currentScene?.scene_name || '无')
const currentScene = computed(() => gameStore.currentScene)

const fetchScenes = async () => {
  try {
    scenes.value = await listScenes()
  } catch (error) {
    console.error('获取场景列表失败', error)
  }
}

const isSceneSelected = (sceneId: string): boolean => {
  return gameStore.currentScene?.id === sceneId
}

const handleSceneClick = async (scene: SceneInfo) => {
  // 无描述时提醒用户
  if (!scene.scene_description?.trim()) {
    uiStore.showInfo({
      title: '提示',
      message: `场景"${scene.scene_name}"暂无描述，选择后不会触发场景旁白`,
      duration: 4000,
    })
  }

  try {
    await selectScene(scene.id)
    gameStore.setCurrentScene(scene)
    if (scene.background) {
      uiStore.setCurrentBackground(scene.background)
    }
    await fetchScenes()
  } catch (error) {
    console.error('选择场景失败', error)
  }
}

const handleWrenchClick = (scene: SceneInfo) => {
  editMode.value = 'update'
  editingSceneId.value = scene.id
  editInitialData.value = {
    sceneName: scene.scene_name,
    sceneImage: scene.background || null,
    sceneDescription: scene.scene_description,
    lighting: scene.lighting,
  }
  showSceneEdit.value = true
}

const handleCreateScene = () => {
  editMode.value = 'create'
  editingSceneId.value = null
  editInitialData.value = undefined
  showSceneEdit.value = true
}

const handleDeleteScene = async () => {
  if (!currentScene.value) return
  if (!(await dialogStore.confirm(`确定要删除场景"${currentScene.value.scene_name}"吗？`))) return

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
  lighting?: LightingParams | null
}) => {
  try {
    if (editMode.value === 'create') {
      await createScene({
        scene_name: data.sceneName,
        scene_description: data.sceneDescription,
        background: data.sceneImage || '',
        lighting: data.lighting ?? null,
      })
    } else {
      if (!editingSceneId.value) return
      await updateScene({
        id: editingSceneId.value,
        scene_name: data.sceneName,
        scene_description: data.sceneDescription,
        background: data.sceneImage || '',
        lighting: data.lighting ?? null,
      })
    }
    showSceneEdit.value = false
    await fetchScenes()

    // 如果更新的是当前选中的场景，立即同步到 gameStore 使光影等参数即时生效
    if (editMode.value === 'update' && editingSceneId.value === gameStore.currentScene?.id) {
      const updatedScene = scenes.value.find(s => s.id === editingSceneId.value)
      if (updatedScene) {
        gameStore.setCurrentScene(updatedScene)
        if (updatedScene.background) {
          uiStore.setCurrentBackground(updatedScene.background)
        }
      }
    }
  } catch (error) {
    console.error('操作失败', error)
  }
}

onMounted(async () => {
  window.addEventListener('background-generated', onBackgroundGenerated)

  try {
    await refreshBackground()
  } catch (error) {
    console.error('加载背景图片失败', error)
  }

  await fetchScenes()

  // 恢复上次选中的场景
  if (gameStore.currentScene?.background) {
    uiStore.setCurrentBackground(gameStore.currentScene.background)
  }
})

onUnmounted(() => {
  window.removeEventListener('background-generated', onBackgroundGenerated)
})

async function fetchBackgrounds(): Promise<BackgroundImageInfo[]> {
  try {
    const data = await getBackgroundImages()
    return data.map((background: BackgroundImageInfo) => ({
      title: background.title || 'Untitled',
      url: background.url || '',
      time: background.time,
    }))
  } catch (error) {
    console.error('Failed to fetch background list:', error)
    return []
  }
}

async function refreshBackground(): Promise<void> {
  const items = await fetchBackgrounds()
  backgroundList.value = items
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
  const allowedExts = ['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.svg', '.tif', '.gif']

  if (!allowedExts.includes(fileExt)) {
    await dialogStore.alert('请上传支持的图片格式: ' + allowedExts.join(', '))
    return
  }

  try {
    const buf = await file.arrayBuffer()
    await uploadBackgroundImage(fileName, new Uint8Array(buf))
    await refreshBackground()
    // 刷新场景列表（后端会自动将新背景注册为场景）
    await fetchScenes()
    if (target) target.value = ''
  } catch (error) {
    console.error('上传失败', error)
    await dialogStore.alert('上传失败，请重试')
  }
}

function updateParticle(value: string): void {
  uiStore.setBackgroundEffect(value)
}

async function handleGenerate(): Promise<void> {
  const prompt = generatePrompt.value.trim()
  if (!prompt || isGenerating.value) return

  isGenerating.value = true
  try {
    await generateBackgroundImage(prompt, userStore.client_id)
    uiStore.showInfo({
      title: '生成已开始',
      message: '背景图正在生成中，请稍候...',
    })
  } catch (e: any) {
    uiStore.showError({
      title: '请求失败',
      message: e.message || '无法开始生成',
    })
    isGenerating.value = false
  }
}

async function handleOpenFolder(): Promise<void> {
  try {
    await openBackgroundsFolder()
  } catch (e: any) {
    uiStore.showError({
      title: '错误',
      message: '无法打开文件夹',
    })
  }
}

function onBackgroundGenerated(event: Event) {
  const detail = (event as CustomEvent).detail
  isGenerating.value = false
  if (detail?.success) {
    generatePrompt.value = ''
  }
  refreshBackground()
  fetchScenes()
}

function handleMeteorFpsChange(value: number) {
  const clampedValue = Math.max(10, Math.min(60, value))
  meteorFpsInput.value = clampedValue
  settingsStore.setMeteorFps(clampedValue)
}

function handleInputBlur() {
  let value = Number(meteorFpsInput.value)
  if (isNaN(value) || value < 10) value = 10
  else if (value > 300) value = 300
  meteorFpsInput.value = value
  settingsStore.setMeteorFps(value)
}

function handleInputEnter() {
  handleInputBlur()
}

watch(meteorFps, (newValue) => {
  meteorFpsInput.value = newValue
})

function handleStarsFpsChange(value: number) {
  const clampedValue = Math.max(10, Math.min(60, value))
  starsFpsInput.value = clampedValue
  settingsStore.setStarsFps(clampedValue)
}

function handleStarsInputBlur() {
  let value = Number(starsFpsInput.value)
  if (isNaN(value) || value < 10) value = 10
  else if (value > 300) value = 300
  starsFpsInput.value = value
  settingsStore.setStarsFps(value)
}

function handleStarsInputEnter() {
  handleStarsInputBlur()
}

watch(starsFps, (newValue) => {
  starsFpsInput.value = newValue
})
</script>
