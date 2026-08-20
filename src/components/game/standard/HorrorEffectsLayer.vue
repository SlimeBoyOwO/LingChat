<template>
  <!-- 恐怖特效层：挂在 GameExtraUI（游戏 UI 最顶层），压过角色立绘与对话框 -->
  <!-- 支持 '+' 组合叠加，如 effect: 'Glitch+BloodDrip+BloodUI' -->
  <div class="pointer-events-none absolute inset-0" style="isolation: isolate">
    <Glitch v-if="active.has('Glitch')" :enabled="true" />
    <Shake v-if="active.has('Shake')" :enabled="true" />
    <Flash v-if="active.has('Flash')" :enabled="true" mode="red" />
    <Flash v-if="active.has('Blackout')" :enabled="true" mode="black" />
    <Tear v-if="active.has('Tear')" :enabled="true" />
    <Static v-if="active.has('Static')" :enabled="true" />
    <Invert v-if="active.has('Invert')" :enabled="true" />
    <BloodDrip v-if="active.has('BloodDrip')" :enabled="true" />
    <Veins v-if="active.has('Veins')" :enabled="true" />
    <Bsod v-if="active.has('BSOD')" :enabled="true" />
    <UiCorrupt v-if="active.has('UiCorrupt')" />
    <UiBlood v-if="active.has('BloodUI')" />
  </div>

  <!-- 突脸惊吓层：最顶层，压过一切 -->
  <Jumpscare />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUIStore } from '../../../stores/modules/ui/ui'
import Glitch from './particles/Glitch.vue'
import Shake from './particles/Shake.vue'
import Flash from './particles/Flash.vue'
import Tear from './particles/Tear.vue'
import Static from './particles/Static.vue'
import Invert from './particles/Invert.vue'
import BloodDrip from './particles/BloodDrip.vue'
import Veins from './particles/Veins.vue'
import Bsod from './particles/Bsod.vue'
import UiCorrupt from './particles/UiCorrupt.vue'
import UiBlood from './particles/UiBlood.vue'
import Jumpscare from './particles/Jumpscare.vue'

const uiStore = useUIStore()

/** 当前生效的特效集合；'none'/空串 = 清空 */
const active = computed<Set<string>>(() => {
  const raw = uiStore.currentBackgroundEffect
  if (!raw || raw === 'none' || raw === 'None') return new Set()
  return new Set(raw.split('+').map((s) => s.trim()).filter(Boolean))
})
</script>

