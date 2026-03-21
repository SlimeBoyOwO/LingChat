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
        <div v-for="setting in settings" :key="setting.key" class="">
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

      <MenuItem title="返回主菜单" size="small">
        <template #header>
          <ArrowBigLeft :size="20" />
        </template>
        <Button type="big" @click="returnToMain">返回主菜单</Button>
      </MenuItem>
    </MenuPage>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { MenuPage, MenuItem } from '../../ui'
import { useStorage } from '@vueuse/core'
import { Slider, Text, Toggle, Button } from '../../base'
import { useUIStore } from '../../../stores/modules/ui/ui'
import type { ConfigItem } from '@/api/services/config'
import SettingItem from '@/components/base/items/SettingItem.vue'
import { getEnvConfigByKey, saveEnvConfigSettings } from '@/api/services/config'
import {
  Zap,
  ClipboardList,
  Star,
  Earth,
  SquareTerminal,
  Settings,
  ArrowBigLeft,
  Rss,
} from 'lucide-vue-next'

const router = useRouter()
const textSpeedSample = ref()
const uiStore = useUIStore()
const settings = ref<Record<string, ConfigItem>>({})

const returnToMain = () => {
  uiStore.toggleSettings(false)
  router.push('/')
}

onMounted(() => {
  loadConfig()
})

const loadConfig = async () => {
  const configKeys = ['USE_PERSISTENT_MEMORY']

  for (const key of configKeys) {
    settings.value[key] = await getEnvConfigByKey(key)
  }
}

const textSpeed = useStorage('lingchat-text-speed', 50)
// 同步 localStorage 中的音量到 Pinia store
watch(
  [textSpeed],
  ([textSpeed]) => {
    uiStore.typeWriterSpeed = textSpeed
    textSpeedSample.value = textSpeed
  },
  { immediate: true },
)

const textSpeedChange = (data: number) => {
  textSpeed.value = data
  textSpeedSample.value = data
  uiStore.typeWriterSpeed = data
}
const animateSwitch = (data: boolean) => {
  console.log(data)
}
const voiceSound = (data: boolean) => {
  uiStore.enableChatEffectSound = data
}

const handleMemorySettingChange = (checked: boolean, setting: ConfigItem) => {
  const newValue = checked ? 'true' : 'false'
  setting.value = newValue

  console.log('changed')

  const formData: Record<string, string> = {}
  Object.entries(settings.value).forEach(([key, config]) => {
    formData[key] = config.value
  })
  saveEnvConfigSettings(formData)
}
</script>

<style scoped>
.settings-text-container {
  position: relative;
  width: 100%;
  height: 100%;
}
</style>
