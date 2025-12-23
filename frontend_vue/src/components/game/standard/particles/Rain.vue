<template>
  <div class="rain-container" ref="containerRef">
    <span
      class="rain-item"
      v-for="(item, idx) in rains"
      :key="idx"
      :style="{
        top: `${item.top}px`,
        left: item.left,
        height: `${item.size}px`,
        opacity: item.opacity,
      }"
    ></span>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'

// 配置参数
const props = defineProps({
  enabled: {
    type: Boolean,
    default: true,
  },
  intensity: {
    type: Number,
    default: 1,
    validator: (value) => value >= 0 && value <= 2,
  },
})

// 生成雨滴速度 (根据强度调整)
const createRainInterval = ref(50)
// 雨滴下落速度
const rainRaceInterval = ref(20)

const rains = ref([])
const rainTimer = ref(null)
const raceTimer = ref(null)
const maxHeight = ref(0)
// 直接引用模板中的根元素
const containerRef = ref(null)

// 根据强度调整参数
watch(
  () => props.intensity,
  (newIntensity) => {
    // 确保强度不会导致间隔小于或等于0
    createRainInterval.value = Math.max(10, 100 - newIntensity * 45)
  },
  { immediate: true }, // 立即执行一次以初始化
)

// 初始化雨滴
const initialRains = () => {
  if (!props.enabled) return

  clearInterval(rainTimer.value)
  rainTimer.value = setInterval(() => {
    const rainItem = {
      top: -100, // 从屏幕外更高一点的位置开始，效果更自然
      left: `${~~(Math.random() * 100)}%`,
      // 速度也应该和强度关联，让雨滴下落更快
      speed: 10 + Math.random() * 10 * props.intensity,
      size: 30 + Math.random() * 40 * props.intensity,
      opacity: 0.2 + Math.random() * 0.5, // 透明度与强度关联性可以减弱，避免太亮
    }
    rains.value.push(rainItem) // 使用 push 比解构赋值性能稍好
  }, createRainInterval.value)
}

// 雨滴运动
const initialRace = () => {
  if (!props.enabled) return

  clearInterval(raceTimer.value)
  raceTimer.value = setInterval(() => {
    // 【修复】使用 filter 方法替代 reduce，代码更清晰，意图更明确
    // 过滤掉所有已经超出屏幕高度的雨滴
    const activeRains = rains.value.filter((rain) => rain.top < maxHeight.value)

    // 只更新仍在屏幕内的雨滴的位置
    activeRains.forEach((rain) => {
      rain.top += rain.speed
    })

    rains.value = activeRains
  }, rainRaceInterval.value)
}

// 【修复】设置最大高度的逻辑
const setMaxHeight = () => {
  // 优先使用父元素的高度，如果获取不到（比如父元素是body），则回退到窗口高度
  // 确保 containerRef.value 存在
  if (containerRef.value && containerRef.value.parentElement) {
    maxHeight.value = containerRef.value.parentElement.clientHeight
  } else {
    maxHeight.value = window.innerHeight
  }
}

onMounted(() => {
  // 使用 nextTick 确保DOM已经完全渲染完毕
  nextTick(() => {
    setMaxHeight()
    if (props.enabled) {
      initialRains()
      initialRace()
    }
  })

  // 监听窗口大小变化
  window.addEventListener('resize', setMaxHeight)
})

onUnmounted(() => {
  clearInterval(rainTimer.value)
  clearInterval(raceTimer.value)
  window.removeEventListener('resize', setMaxHeight)
})

// 监听启用状态变化
watch(
  () => props.enabled,
  (newVal) => {
    if (newVal) {
      // 重新启动时也需要确保高度是正确的
      setMaxHeight()
      initialRains()
      initialRace()
    } else {
      clearInterval(rainTimer.value)
      clearInterval(raceTimer.value)
      rains.value = []
    }
  },
)
</script>

<style scoped>
.rain-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: -1;
  overflow: hidden;
}

.rain-item {
  position: absolute;
  display: inline-block;
  width: 2px;
  background: linear-gradient(rgba(255, 255, 255, 0.3), rgba(255, 255, 255, 0.6));
}
</style>
