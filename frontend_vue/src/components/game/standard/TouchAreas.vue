<template>
  <div v-if="isVisible" class="touch-areas-container">

  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'


import { useGameStore } from '@/stores/modules/game'

interface Props {
  isVisible?: boolean
}


const props = withDefaults(defineProps<Props>(), {
  isVisible: true,
})

const gameStore = useGameStore();

const emit = defineEmits(['area-clicked'])

const handleTouchAreaClicked = (area: string) => {
  // 检查当前是否处于触摸模式
  if (gameStore.command === 'touch') {
    alert(`touched ${area} area`)
  }
}
</script>

<style scoped>

</style>
