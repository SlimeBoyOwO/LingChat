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

      <MenuItem title="页面切换动画" size="small">
        <template #header>
          <Star :size="20" />
        </template>
        <Toggle @change="animateSwitch">启用动画效果</Toggle>
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

      <MenuItem title="当前使用的AI大模型" size="small">
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
import { ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { MenuPage, MenuItem } from '../../ui'
import { useStorage } from '@vueuse/core'
import { Slider, Text, Toggle, Button } from '../../base'
import { useUIStore } from '../../../stores/modules/ui/ui'
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

const returnToMain = () => {
  uiStore.toggleSettings(false)
  router.push('/')
}

// 使用 VueUse 的 useStorage 持久化存储音量设置
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
</script>

<style scoped>
.settings-text-container {
  position: relative;
  width: 100%;
  height: 100%;
}
</style>
