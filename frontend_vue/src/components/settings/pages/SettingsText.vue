<template>
  <MenuPage>
    <MenuItem title="⚡ 文字显示速度">
      <Slider @change="textSpeedChange">慢/快</Slider>
    </MenuItem>

    <MenuItem title="📝 显示文字样本">
      <Text :speed="textSpeedSample">Ling Chat: 测试文本显示速度</Text>
    </MenuItem>

    <MenuItem title="✨ 页面切换动画" size="small">
      <Toggle @change="animateSwitch">启用动画效果</Toggle>
    </MenuItem>

    <MenuItem title="🌏 语音音效开关" size="small">
      <Toggle @change="voiceSound">启用无vits时的对话音效</Toggle>
    </MenuItem>

    <MenuItem title="🎛️ 指令盘开关" size="small">
      <Toggle @change="commandWheelToggle">显示指令盘</Toggle>
    </MenuItem>

    <MenuItem title="✨ WebSocket通信状态" size="small">
      <p>√ 连接正常</p>
    </MenuItem>

    <MenuItem title="⚙ 当前使用的AI大模型" size="small">
      <p>DeepSeek V3</p>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { MenuPage } from '../../ui'
import { MenuItem } from '../../ui'
import { Slider } from '../../base'
import { Text } from '../../base'
import { Toggle } from '../../base'
import { useUIStore } from '../../../stores/modules/ui/ui'

const textSpeedSample = ref()

const uiStore = useUIStore()

const textSpeedChange = (data: number) => {
  textSpeedSample.value = data
  uiStore.typeWriterSpeed = data
}
const animateSwitch = (data: boolean) => {
  console.log(data)
}
const voiceSound = (data: boolean) => {
  uiStore.enableChatEffectSound = data
}
const commandWheelToggle = (data: boolean) => {
  uiStore.toggleCommandWheel(data)
}
</script>

<style scoped>
/* --- 文本设置页面新样式 --- */
.settings-columns {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0px;
  height: 100%;
}

.setting-item {
  margin-bottom: 25px;
  width: 100%;
  max-width: 900px;
}
</style>
