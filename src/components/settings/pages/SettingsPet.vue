<template>
  <MenuPage>
    <!-- 桌宠尺寸 -->
    <MenuItem title="桌宠大小" size="small">
      <template #header>
        <Ruler :size="20" class="text-blue-400" />
      </template>
      <div class="flex flex-col gap-2 w-full">
        <Slider v-model="petScale" :min="0.5" :max="2.0" :step="0.05" @change="updateScale">
          {{ percentLabel }}/恢复默认
        </Slider>
        <div class="flex items-center gap-3 mt-1">
          <Button type="big" class="flex-grow" @click="resetScale">恢复默认尺寸 (100%)</Button>
        </div>
      </div>
    </MenuItem>

    <!-- 桌宠音量 -->
    <MenuItem title="桌宠音量" size="small">
      <template #header>
        <Volume2 :size="20" class="text-indigo-400" />
      </template>
      <div class="flex flex-col gap-2 w-full">
        <Slider v-model="characterVolume" :min="0" :max="100" :step="1" @change="updateVolume">
          {{ characterVolume }}%/强
        </Slider>
        <div class="flex items-center gap-3 mt-1">
          <Button type="big" class="flex-grow" @click="resetVolume">恢复默认音量 (80%)</Button>
        </div>
      </div>
    </MenuItem>

    <!-- 桌宠背景粒子效果 -->
    <MenuItem title="背景粒子" size="small">
      <template #header>
        <Sparkles :size="20" class="text-yellow-400" />
      </template>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <Button
          v-for="opt in particleOptions"
          :key="opt.value"
          type="big"
          class="flex-1 min-w-30"
          :class="{ active: currentParticle === opt.value }"
          @click="selectParticle(opt.value)"
        >
          {{ opt.label }}
        </Button>
      </div>
    </MenuItem>

    <!-- 桌宠运行模式 -->
    <MenuItem title="启动桌宠模式" size="small">
      <template #header>
        <Cat :size="20" class="text-pink-400" />
      </template>
      <div class="flex items-center gap-3">
        <Button type="big" class="flex-grow" @click="startPetMode">
          进入透明桌宠模式
        </Button>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useSettingsStore } from '../../../stores/modules/settings'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { Button, Slider } from '../../base'
import { MenuItem, MenuPage } from '../../ui'
import { Ruler, Volume2, Sparkles, Cat } from 'lucide-vue-next'
import { getCurrentWindow } from '@tauri-apps/api/window'

const settingsStore = useSettingsStore()
const uiStore = useUIStore()
const router = useRouter()

const petScale = computed({
  get: () => settingsStore.pet?.scale ?? 1.0,
  set: (val: number) => {
    settingsStore.setPetScale(val)
  },
})

const characterVolume = computed({
  get: () => settingsStore.characterVolume,
  set: (val: number) => {
    settingsStore.update('audio.characterVolume', val)
  },
})

const percentLabel = computed(() => {
  return `${Math.round(petScale.value * 100)}%`
})

const updateScale = async (value: number) => {
  settingsStore.setPetScale(value)
  await getCurrentWindow().emit('pet-scale-changed', { scale: value })
}

const resetScale = async () => {
  await updateScale(1.0)
}

const updateVolume = async (value: number) => {
  settingsStore.update('audio.characterVolume', value)
  await getCurrentWindow().emit('pet-volume-changed', { volume: value })
}

const resetVolume = async () => {
  await updateVolume(80)
}

const currentParticle = computed(() => uiStore.currentBackgroundEffect)

const particleOptions = [
  { label: '无效果', value: 'None' },
  { label: '流星星空', value: 'StarField' },
  { label: '星辉之雨', value: 'BA' },
]

const selectParticle = async (value: string) => {
  uiStore.setBackgroundEffect(value)
  await getCurrentWindow().emit('background-effect-changed', { effect: value })
}

const startPetMode = () => {
  uiStore.toggleSettings(false)
  router.push('/pet')
}
</script>

<style scoped>
.active {
  color: var(--accent-color) !important;
  border: 1px solid var(--accent-color) !important;
  background-color: rgba(255, 255, 255, 0.1) !important;
}
</style>
