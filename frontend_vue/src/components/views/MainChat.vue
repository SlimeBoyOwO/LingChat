<template>
  <div class="main-box">
    <!-- 左上角番茄钟开关与面板 -->
    <FreeModeTools />
    <GameBackground></GameBackground>
    <GameRolesStage ref="gameAvatarRef" @audio-ended="handleAudioFinished" />
    <GameDialog
      ref="gameDialogRef"
      @player-continued="manualTriggerContinue"
      @dialog-proceed="resetInteraction"
    />

    <!-- 场景控制区域 - 添加在现有按钮附近 -->
    <div id="scene-panel" class="scene-controls">
      <el-button size="small" @click="openSceneDialog">
        {{ currentScene ? '切换场景' : '加载场景' }}
      </el-button>
      <el-button v-if="currentScene" size="small" type="danger" @click="handleClearScene">
        清除场景
      </el-button>
      <span v-if="currentScene" class="scene-indicator">
        当前场景：{{ getSceneDisplayName(currentScene) }}
      </span>
    </div>

    <!-- 原有的菜单按钮 -->
    <div id="menu-panel">
      <Button
        type="nav"
        icon="play"
        @click="switchAutoMode"
        :class="[{ active: uiStore.autoMode }]"
        v-show="uiStore.showSettings !== true"
      >
        <h3>自动</h3>
      </Button>
      <Button type="nav" icon="text" @click="openSettings" v-show="uiStore.showSettings !== true">
        <h3>菜单</h3>
      </Button>
    </div>

    <!-- 场景选择对话框 -->
    <el-dialog v-model="sceneDialogVisible" title="选择场景" width="400px">
      <el-select
        v-model="selectedScene"
        placeholder="请选择场景"
        style="width: 100%"
        :loading="isLoadingScenes"
      >
        <el-option
          v-for="scene in scenes"
          :key="scene.filename"
          :label="scene.description"
          :value="scene.filename"
        />
      </el-select>
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="sceneDialogVisible = false">取消</el-button>
          <el-button type="primary" @click="confirmLoadScene">确定</el-button>
        </span>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import FreeModeTools from '@/components/tools/FreeModeTools.vue'
import { useUIStore } from '../../stores/modules/ui/ui'
import { useGameStore } from '../../stores/modules/game'
import { useUserStore } from '../../stores/modules/user/user'
import { GameBackground, GameRolesStage } from '../game/standard'
import { GameDialog } from '../game/standard'
import { Button } from '../base'
import { ElMessage, ElDialog, ElSelect, ElOption } from 'element-plus'
import { listScenes, loadScene, clearScene, type SceneInfo } from '@/api/scene' // 需要创建这个 API 文件
const uiStore = useUIStore()
const gameStore = useGameStore()
const userStore = useUserStore()

// 场景相关状态
const sceneDialogVisible = ref(false)
const scenes = ref<SceneInfo[]>([])
const selectedScene = ref<string>('')
const isLoadingScenes = ref(false)
const currentScene = ref<SceneInfo | null>(null) // 当前加载的场景

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
const getSceneDisplayName = (scene: SceneInfo | null) => {
  if (!scene) return ''
  // 去掉扩展名，例如 "海边.png" -> "海边"
  return scene.filename.replace(/\.[^/.]+$/, '')
}
// 打开场景选择对话框
const openSceneDialog = () => {
  fetchScenes()
  sceneDialogVisible.value = true
}

// 确认加载场景
const confirmLoadScene = async () => {
  if (!selectedScene.value) {
    ElMessage.warning('请选择一个场景')
    return
  }
  try {
    await loadScene(selectedScene.value)
    const scene = scenes.value.find((s) => s.filename === selectedScene.value)
    currentScene.value = scene || null
    ElMessage.success(`场景“${scene?.description}”已加载`)
    sceneDialogVisible.value = false
  } catch (error) {
    ElMessage.error('加载场景失败')
  }
}

// 清除场景
const handleClearScene = async () => {
  try {
    await clearScene()
    currentScene.value = null
    selectedScene.value = ''
    ElMessage.success('已清除场景，返回自由对话模式')
  } catch (error) {
    ElMessage.error('清除场景失败')
  }
}

const gameDialogRef = ref<InstanceType<typeof GameDialog> | null>(null)

const openSettings = () => {
  uiStore.toggleSettings(true)
  uiStore.setSettingsTab('text')
}

const switchAutoMode = () => {
  uiStore.autoMode = !uiStore.autoMode
}

const runInitialization = async () => {
  const userId = '1' // TODO: 获取真实 userId

  try {
    await gameStore.initializeGame(userStore.client_id, userId)
  } catch (error) {
    console.log(error)
  }
}

// 初始化游戏信息
onMounted(() => {
  if (userStore.client_id !== '') {
    runInitialization()
  }
})

// 监听 client_id 的变化
watch(
  () => userStore.client_id,
  (newId) => {
    if (newId) {
      runInitialization()
    }
  },
)

/* 以下代码为自动AUTO模式逻辑 比较复杂 */
// 1. 用于存储 setTimeout 返回的 ID
let timerId: any = null
// 2. 状态标志，记录 continue() 是否已被调用
const isContinueTriggered = ref(false)

// 在新交互开始前调用的重置函数
const resetInteraction = () => {
  isContinueTriggered.value = false
  if (timerId) {
    clearTimeout(timerId)
    timerId = null
  }
}

// 自动播放功能
const handleAudioFinished = () => {
  if (!uiStore.autoMode) return
  if (isContinueTriggered.value) {
    console.log('父组件：音频结束了，但用户已手动继续，不做任何事。')
    return
  }
  if (gameStore.currentStatus !== 'responding') return

  if (timerId) clearTimeout(timerId)

  timerId = setTimeout(() => {
    if (gameDialogRef.value) {
      const needWait = gameDialogRef.value.continueDialog(false)
      if (needWait) {
        handleAudioFinished()
      }
    } else {
      console.error('无法找到 GameDialog 的实例。')
    }
  }, 1000)
}

// 用户手动触发的函数
const manualTriggerContinue = () => {
  // 5. 立即清除定时器，阻止其后续执行
  console.log('用户主动点击了')
  if (timerId) {
    clearTimeout(timerId)
    timerId = null
    console.log('父组件：已取消自动继续的定时器。')
  }

  // 6. 检查状态，防止重复执行
  if (!isContinueTriggered.value) {
    isContinueTriggered.value = true // 设置标志
  } else {
    console.log('父组件：用户重复点击，但方法已执行过，不再调用。')
  }
}
</script>

<style>
.main-box {
  position: absolute;
  height: 100%;
  width: 100%;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  align-items: center;
  overflow: hidden;
}

#menu-panel {
  display: flex;
  position: fixed;
  top: 15px;
  right: 20px;
  z-index: 1000;
}
.scene-controls {
  position: fixed;
  bottom: 80px; /* 根据聊天输入框高度调整 */
  left: 20px;
  display: flex;
  gap: 8px;
  align-items: center;
  background: rgba(0, 0, 0, 0.5);
  padding: 8px 12px;
  border-radius: 20px;
  backdrop-filter: blur(5px);
  z-index: 100;
}

.scene-indicator {
  color: #fff;
  font-size: 14px;
  margin-left: 8px;
}
</style>
