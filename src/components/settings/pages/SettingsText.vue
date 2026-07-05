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

      <MenuItem title="内联动作文本" size="small">
        <template #header>
          <AlignJustify :size="20" />
        </template>
        <Toggle :checked="settingsStore.text.inlineMotionText" @change="toggleInlineMotionText">
          开启后动作文本将与台词同时显示，无需二次点击
        </Toggle>
      </MenuItem>

      <MenuItem title="久坐喝水提醒" size="small">
        <template #header>
          <GlassWater :size="20" />
        </template>
        <Toggle :checked="settingsStore.text.sedentaryReminder" @change="toggleSedentaryReminder">
          开启后每40分钟发送提醒一下久坐哦，只是健康小助手捏
        </Toggle>
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

      <!-- ─── 语音缓存 ──────────────────────────────── -->
      <MenuItem title="语音缓存" size="small">
        <template #header>
          <HardDrive :size="20" />
        </template>
        <div class="space-y-2 w-full">
          <div class="flex items-center justify-between text-base">
            <span class="text-gray-50">当前缓存</span>
            <span class="text-gray-50 font-medium">{{ ttsCacheSize }}</span>
          </div>
          <div class="text-gray-50/70 text-xs">
            {{ ttsCacheFiles }} 个文件
          </div>
          <div class="flex gap-3 pt-1">
            <Button type="big" @click="checkTtsCache">
              <RefreshCw :size="16" class="mr-1" /> 检查缓存
            </Button>
            <Button type="big" @click="handleClearTtsCache">
              <Trash2 :size="16" class="mr-1" /> 清理语音缓存
            </Button>
          </div>
        </div>
      </MenuItem>

      <!-- ─── 版本更新 ──────────────────────────────── -->
      <MenuItem title="版本更新" size="small">
        <template #header>
          <RefreshCw :size="20" :class="{ 'animate-spin': updateChecking }" />
        </template>
        <div class="space-y-2 w-full">
          <div class="flex items-center justify-between text-base">
            <span class="text-gray-50">当前版本</span>
            <span class="text-gray-50">v{{ currentAppVersion }}</span>
          </div>
          <div v-if="updateDataInfo" class="flex items-center justify-between">
            <span class="text-gray-50">数据版本</span>
            <span class="text-gray-50">v{{ updateDataInfo.currentVersion }}</span>
          </div>
          <div v-if="updateLatestVersion" class="flex items-center justify-between">
            <span class="text-gray-50">最新版本</span>
            <span class="text-gray-50 font-bold">{{ updateLatestVersion }}</span>
          </div>
          <div v-if="updateStatusText" :class="updateStatusColor">
            {{ updateStatusText }}
          </div>
          <div class="flex gap-3 pt-1">
            <Button type="big" @click="handleCheckUpdate" :disabled="updateChecking">
              {{ updateChecking ? '检查中...' : '检查更新' }}
            </Button>
            <Button v-if="updateAvailable" type="big" @click="handleDoUpdate"> 立即更新 </Button>
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

      <!-- ─── 局域网同步 ──────────────────────────────── -->
      <MenuItem title="局域网数据同步" size="small">
        <template #header>
          <Wifi :size="20" />
        </template>
        <div class="space-y-2 w-full">
          <p class="text-gray-50 text-sm">
            在同一局域网内的设备之间同步 data 文件夹（游戏存档、语音、截图等）。
          </p>
          <div class="flex gap-3 pt-1">
            <Button type="big" @click="openLanSync"> 打开局域网同步 </Button>
          </div>
          <!-- 局域网同步对话框 -->
          <LanSyncDialog
            :visible="lanSync.dialogVisible.value"
            :view="lanSyncView"
            :phase="lanSync.phase.value"
            :server-port="lanSync.serverPort.value"
            :peers="lanSync.peers.value"
            :sync-plan="lanSync.syncPlan.value"
            :progress="lanSync.progress.value"
            :last-result="lanSync.lastResult.value"
            :error-message="lanSync.errorMessage.value"
            @close="lanSync.closeDialog()"
            @rescan="lanSync.scanPeers()"
            @pull="
              (peer) => {
                lanSync.selectPeer(peer)
                lanSync.planPull()
              }
            "
            @push="
              (peer) => {
                lanSync.selectPeer(peer)
                lanSync.planPush()
              }
            "
            @confirm="handleLanSyncConfirm"
            @cancel="lanSync.reset()"
            @restart="lanSync.restart()"
          />
        </div>
      </MenuItem>
    </MenuPage>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
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
  Wifi,
  AlignJustify,
  GlassWater,
  HardDrive,
  Trash2,
} from 'lucide-vue-next'
import { reactivateTTS, clearTtsCache } from '@/api/services/game-info'
import { invoke } from '@tauri-apps/api/core'
import { useUpdater } from '@/composables/useUpdater'
import { useLanSync } from '@/composables/useLanSync'
import { getVersion } from '@tauri-apps/api/app'
import UpdateDialog from '@/components/UpdateDialog.vue'
import LanSyncDialog from '@/components/LanSyncDialog.vue'
import type { DialogView } from '@/types/lanSync'

const router = useRouter()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()
const userStore = useUserStore()
const gameStore = useGameStore()
const dialogStore = useDialogStore()
const envSettings = ref<Record<string, ConfigItem>>({})
const ttsCacheSize = ref('0 B')
const ttsCacheFiles = ref(0)

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
  if (
    updateDataInfo.value &&
    !updateDataInfo.value.available &&
    updateDataInfo.value.currentVersion > 0
  )
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
      updateLatestVersion.value =
        updateLatestVersion.value || `(数据 v${updateDataInfo.value.newVersion})`
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

// ─── 局域网同步 ────────────────────────────────────────────────

const lanSync = useLanSync()
const lanSyncView = ref<DialogView>('device-list')

// 监听阶段变化，自动切换视图
watch(
  () => lanSync.phase.value,
  (newPhase) => {
    switch (newPhase) {
      case 'idle':
      case 'scanning':
        lanSyncView.value = 'device-list'
        break
      case 'planning':
        lanSyncView.value = 'sync-plan'
        break
      case 'executing':
        lanSyncView.value = 'progress'
        break
      case 'complete':
      case 'error':
        lanSyncView.value = 'result'
        break
    }
  },
)

async function openLanSync() {
  lanSync.init()
  await lanSync.openDialog()
  lanSyncView.value = 'device-list'
}

async function handleLanSyncConfirm() {
  const plan = lanSync.syncPlan.value
  if (!plan) return
  lanSyncView.value = 'progress'
  if (plan.direction === 'pull') {
    await lanSync.executePull()
  } else {
    await lanSync.executePush()
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
  checkTtsCache()
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

const toggleInlineMotionText = (data: boolean) => {
  settingsStore.update('text.inlineMotionText', data)
}

const toggleSedentaryReminder = (data: boolean) => {
  settingsStore.update('text.sedentaryReminder', data)
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

const handleClearTtsCache = async () => {
  try {
    const result = await clearTtsCache()
    await checkTtsCache()
    uiStore.showNotification({
      type: result.success ? 'success' : 'warning',
      title: result.success ? '清理成功' : '清理完成',
      message: result.message,
      duration: 3000,
      skipTipsCheck: true,
    })
  } catch (error: any) {
    uiStore.showNotification({
      type: 'error',
      title: '清理失败',
      message: error.message || '清理TTS缓存失败',
      duration: 3000,
      skipTipsCheck: true,
    })
  }
}

async function checkTtsCache() {
  try {
    const result = await invoke<{ size: number; files: number }>('get_tts_cache_info')
    ttsCacheFiles.value = result.files
    ttsCacheSize.value = formatBytes(result.size)
  } catch (error: any) {
    console.error('获取TTS缓存信息失败:', error)
    ttsCacheSize.value = '未知'
    ttsCacheFiles.value = 0
  }
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i]
}
</script>

<style scoped>
.settings-text-container {
  position: relative;
  width: 100%;
  height: 100%;
}
</style>
