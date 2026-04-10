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

      <MenuItem title="返回主菜单" size="small">
        <template #header>
          <ArrowBigLeft :size="20" />
        </template>
        <div class="flex gap-3">
          <Button type="big" @click="returnToMain">返回主菜单</Button>
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
} from 'lucide-vue-next'

const router = useRouter()
const uiStore = useUIStore()
const settingsStore = useSettingsStore()
const userStore = useUserStore()
const gameStore = useGameStore()
const envSettings = ref<Record<string, ConfigItem>>({})

// 判断是否在自由对话模式（没有运行剧本）
const isFreeDialogMode = computed(() => gameStore.runningScript === null)

const returnToMain = () => {
  uiStore.toggleSettings(false)
  router.push('/')
}

const handleClearHistory = async () => {
  // 提示用户保存
  const confirmed = window.confirm(
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
    uiStore.currentBackground = '@/assets/images/default_bg.jpg'
    uiStore.setBackgroundEffect('none')
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
  const configKeys = ['USE_PERSISTENT_MEMORY']
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
</script>

<style scoped>
.settings-text-container {
  position: relative;
  width: 100%;
  height: 100%;
}
</style>
