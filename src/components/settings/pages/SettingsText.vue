<template>
  <div class="settings-text-container">
    <MenuPage>
      <MenuItem title="文字显示速度">
        <template #header>
          <Zap :size="20" />
        </template>
        <Slider @change="textSpeedChange" v-model="textSpeed">慢/快</Slider>
      </MenuItem>

      <MenuItem title="显示文字样本">
        <template #header>
          <ClipboardList :size="20" />
        </template>
        <Text :speed="textSpeedSample">Ling Chat: 测试文本显示速度</Text>
      </MenuItem>

      <MenuItem title="启用永久记忆" size="small">
        <div v-for="setting in envSettings" :key="setting.key" class="">
          <!-- 使用 SettingItem 组件渲染不同类型的输入控件 -->
          <Toggle
            :checked="setting.value.toLowerCase() === 'true'"
            @change="handleMemorySettingChange($event, setting)"
          >
            开启后记忆将会自动压缩
          </Toggle>
        </div>
        <template #header>
          <Star :size="20" />
        </template>
      </MenuItem>

      <MenuItem title="语音音效开关" size="small">
        <template #header>
          <Earth :size="20" />
        </template>
        <Toggle @change="voiceSound">启用无vits时的对话音效</Toggle>
      </MenuItem>

      <MenuItem title="WebSocket通信状态" size="small">
        <template #header>
          <Rss :size="20" />
        </template>
        <p>√ 连接正常</p>
      </MenuItem>

      <MenuItem title="当前使用的AI大模型（这里是假的嘻嘻）" size="small">
        <template #header>
          <Settings :size="20" />
        </template>
        <p>DeepSeek V3</p>
      </MenuItem>

      <MenuItem title="语音推理引擎下载（SBV2）" size="small">
        <template #header>
          <Download :size="20" />
        </template>
        <div class="flex gap-3">
          <Button
            type="big"
            @click="
              openWebsite(
                'https://www.modelscope.cn/models/lingchat-research-studio/Style-Bert-VITS2-micro-CPU-infer/files',
              )
            "
            >CPU推理</Button
          >
          <Button
            type="big"
            @click="
              openWebsite(
                'https://www.modelscope.cn/models/lingchat-research-studio/Style-Bert-VITS2-micro-NVIDIA-infer/files',
              )
            "
            >N卡推理</Button
          >
          <Button
            type="big"
            @click="
              openWebsite(
                'https://www.modelscope.cn/models/lingchat-research-studio/Style-Bert-VITS2-micro-Directml-infer/files',
              )
            "
            >A卡推理</Button
          >
        </div>
      </MenuItem>

      <!-- ─── 版本更新 ──────────────────────────────── -->
      <MenuItem title="版本更新" size="small">
        <template #header>
          <RefreshCw :size="20" :class="{ 'animate-spin': updateChecking }" />
        </template>
        <div class="space-y-2 w-full">
          <div class="flex items-center justify-between text-sm">
            <span class="text-slate-500">当前版本</span>
            <span class="font-mono text-slate-700">{{ currentAppVersion }}</span>
          </div>
          <div v-if="updateDataInfo" class="flex items-center justify-between text-sm">
            <span class="text-slate-500">数据版本</span>
            <span class="font-mono text-slate-700">v{{ updateDataInfo.currentVersion }}</span>
          </div>
          <div v-if="updateLatestVersion" class="flex items-center justify-between text-sm">
            <span class="text-slate-500">最新版本</span>
            <span class="font-mono text-cyan-600 font-bold">{{ updateLatestVersion }}</span>
          </div>
          <div v-if="updateStatusText" class="text-xs" :class="updateStatusColor">
            {{ updateStatusText }}
          </div>
          <div class="flex gap-3 pt-1">
            <Button type="big" @click="handleCheckUpdate" :disabled="updateChecking">
              {{ updateChecking ? '检查中...' : '检查更新' }}
            </Button>
            <Button
              v-if="updateAvailable"
              type="big"
              @click="handleDoUpdate"
            >
              立即更新
            </Button>
          </div>
           <UpdateDialog
            :visible="showUpdateInlineDialog"
            :phase="updatePhase"
            :app-version="updateAppVersion"
            :app-release-notes="updateAppReleaseNotes"
            :data-info="updateDataInfo"
            :data-progress="updateDataProgress"
            :error-message="updateErrorMessage"
            @update="handleInstallFromSettings"
            @later="showUpdateInlineDialog = false"
            @close="showUpdateInlineDialog = false"
          />
        </div>
      </MenuItem>

      <MenuItem title="返回主菜单" size="small">
        <template #header>
          <ArrowBigLeft :size="20" />
        </template>
        <div class="flex gap-3">
          <Button type="big" @click="returnToMain">返回主菜单</Button>
          <Button type="big" @click="refreshTTS">刷新TTS服务</Button>
          <Button v-if="isFreeDialogMode" type="big" variant="danger" @click="handleClearHistory"
            >清除历史对话</Button
          >
        </div>
      </MenuItem>
    </MenuPage>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { MenuPage, MenuItem } from '../../ui'
import { Slider, Text, Toggle, Button } from '../../base'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useDialogStore } from '../../../stores/modules/ui/dialog'
import { useSettingsStore } from '../../../stores/modules/settings'
import { useUserStore } from '../../../stores/modules/user/user'
import { useGameStore } from '../../../stores/modules/game'
import type { ConfigItem } from '@/api/services/config'
import { getEnvConfigByKey, saveEnvConfigSettings } from '@/api/services/config'
import { clearChatHistory } from '@/api/services/history'
import {
  Zap,
  ClipboardList,
  Star,
  Earth,
  Settings,
  ArrowBigLeft,
  Rss,
  Download,
  RefreshCw,
} from 'lucide-vue-next'
import { reactivateTTS } from '@/api/services/game-info'
import { useUpdater } from '@/composables/useUpdater'
import { getVersion } from '@tauri-apps/api/app'
import UpdateDialog from '@/components/UpdateDialog.vue'

const router = useRouter()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()
const userStore = useUserStore()
const gameStore = useGameStore()
const dialogStore = useDialogStore()
const envSettings = ref<Record<string, ConfigItem>>({})

// 判断是否在自由对话模式（没有运行剧本）
const isFreeDialogMode = computed(() => gameStore.runningScript === null)

// ─── 更新检查 ────────────────────────────────────────────────

const updater = useUpdater()
const {
  phase: updatePhase,
  appVersion: updateAppVersion,
  appReleaseNotes: updateAppReleaseNotes,
  dataInfo: updateDataInfo,
  dataProgress: updateDataProgress,
  errorMessage: updateErrorMessage,
} = updater

const currentAppVersion = ref('0.1.0')
const updateLatestVersion = ref('')
const updateChecking = ref(false)
const showUpdateInlineDialog = ref(false)

const updateAvailable = computed(() => updateLatestVersion.value !== '')

const updateStatusText = computed(() => {
  if (updateAvailable.value) return '发现新版本可用！'
  if (updateDataInfo.value && !updateDataInfo.value.available && updateDataInfo.value.currentVersion > 0)
    return '✓ 已是最新版本'
  return ''
})

const updateStatusColor = computed(() => {
  if (updateAvailable.value) return 'text-amber-600'
  return 'text-green-600'
})

async function loadAppVersion() {
  try {
    currentAppVersion.value = await getVersion()
  } catch {
    // 使用默认值
  }
}

async function handleCheckUpdate() {
  updateChecking.value = true
  updateLatestVersion.value = ''
  try {
    const hasUpdate = await updater.checkForUpdates()
    if (hasUpdate) {
      updateLatestVersion.value = updateAppVersion.value || updatePhase.value
    }
    // 即使没有 app 更新，也同步 data 版本信息
    if (updateDataInfo.value && updateDataInfo.value.available) {
      updateLatestVersion.value = updateLatestVersion.value || `(数据 v${updateDataInfo.value.newVersion})`
    }
  } catch (e) {
    console.debug('[SettingsText] 更新检查跳过 (无 Release):', String(e).slice(0, 80))
  } finally {
    updateChecking.value = false
  }
}

async function handleDoUpdate() {
  showUpdateInlineDialog.value = true
}

async function handleInstallFromSettings() {
  try {
    await updater.installAllUpdates()
    updateLatestVersion.value = ''
  } catch {
    // 错误通过 phase 反映
  }
}

// 加载版本号
loadAppVersion()

const returnToMain = () => {
  uiStore.toggleSettings(false)
  router.push('/')
}

const handleClearHistory = async () => {
  // 提示用户保存
  const confirmed = await dialogStore.confirm(
    '清除历史对话将丢失当前所有对话记录，建议先存档。\n\n是否已存档或确认清除？',
  )
  if (!confirmed) return

  try {
    // 调用后端清除对话历史
    await clearChatHistory(userStore.user_id.toString())

    // 清除前端状态
    gameStore.clearDialogHistory()
    gameStore.currentStatus = 'input'
    gameStore.currentLine = ''

    // 重置在场角色列表为主角色（与后端对齐）
    if (gameStore.mainRoleId !== -1) {
      gameStore.presentRoleIds = [gameStore.mainRoleId]
      gameStore.currentInteractRoleId = gameStore.mainRoleId
    }

    // 重置 UI 状态
    uiStore.currentBackgroundMusic = 'None'
    uiStore.currentAvatarAudio = 'None'
    uiStore.bgMusicPaused = false
    uiStore.bgMusicStoped = true

    // 清除运行中的剧本状态
    gameStore.exitStoryMode()

    uiStore.showNotification({
      type: 'success',
      title: '清除成功',
      message: '对话历史已清除',
      duration: 3000,
      skipTipsCheck: true,
    })
  } catch (error: any) {
    uiStore.showNotification({
      type: 'error',
      title: '清除失败',
      message: error.message || '清除历史对话失败',
      duration: 3000,
      skipTipsCheck: true,
    })
  }
}

onMounted(() => {
  loadConfig()
})

const loadConfig = async () => {
  const configKeys = ['features.use_persistent_memory']
  for (const key of configKeys) {
    envSettings.value[key] = await getEnvConfigByKey(key)
  }
}

// 使用 settings store 的文字速度
const textSpeed = computed({
  get: () => settingsStore.textSpeed,
  set: (val: number) => settingsStore.update('text.speed', val),
})

// 文字样本速度（响应式）
const textSpeedSample = ref<number>(settingsStore.textSpeed)

const textSpeedChange = (data: number) => {
  settingsStore.update('text.speed', data)
  textSpeedSample.value = data
}

const voiceSound = (data: boolean) => {
  settingsStore.update('audio.chatEffectSound', data)
}

const handleMemorySettingChange = (checked: boolean, setting: ConfigItem) => {
  const newValue = checked ? 'true' : 'false'
  setting.value = newValue

  const formData: Record<string, string> = {}
  Object.entries(envSettings.value).forEach(([key, config]) => {
    formData[key] = config.value
  })
  saveEnvConfigSettings(formData)
}

const openWebsite = (url: string) => {
  window.open(url, '_blank') // '_blank' 表示在新窗口中打开
}

const refreshTTS = async () => {
  try {
    await reactivateTTS()
    await dialogStore.alert('刷新TTS成功，将会在TTS可用的时候自动调用')
  } catch (error) {
    await dialogStore.alert('刷新TTS失败')
  }
}
</script>

<style scoped>
.settings-text-container {
  position: relative;
  width: 100%;
  height: 100%;
}
</style>
