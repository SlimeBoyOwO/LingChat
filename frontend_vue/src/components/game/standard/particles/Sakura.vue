<template>
  <div class="petal-container" ref="containerRef">
    <div
      class="petal"
      v-for="(petal, index) in petals"
      :key="index"
      :style="{
        width: `${petal.size}px`,
        height: `${petal.size}px`,
        left: `${petal.left}px`,
        top: `${petal.top}px`,
        opacity: petal.opacity,
        background: `linear-gradient(135deg, hsl(${petal.hue}, 100%, 85%), hsl(${petal.hue}, 100%, 75%))`,
        animation: `fall-${petal.id} ${petal.duration}s linear ${petal.delay}s infinite`,
      }"
    ></div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from "vue";

// 花瓣接口定义
interface Petal {
  id: string;
  size: number;
  left: number;
  top: number;
  opacity: number;
  hue: number;
  duration: number;
  delay: number;
  horizontalMovement: number;
  styleSheet?: HTMLStyleElement;
}

// 组件属性定义
interface Props {
  enabled?: boolean;
  intensity?: number;
}

// 默认属性值
const props = withDefaults(defineProps<Props>(), {
  enabled: true,
  intensity: 1,
});

// 响应式数据
const petals = ref<Petal[]>([]);
const containerRef = ref<HTMLElement | null>(null);
const maxHeight = ref(0);

// 花瓣数量根据强度调整
const petalCount = ref(Math.floor(25 * props.intensity));

// 生成随机ID
const generateId = (): string => {
  return Math.random().toString(36).substr(2, 9);
};

// 创建单个花瓣
const createPetal = (): Petal => {
  const size = Math.random() * 10 + 10; // 15-25px
  const left = Math.random() * window.innerWidth;
  const top = -30;
  const duration = Math.random() * 10 + 15; // 15-25秒
  const delay = Math.random() * 10; // 0-5秒延迟
  const opacity = Math.random() * 0.5 + 0.4; // 0.3-0.8透明度
  const hue = Math.random() * 10 + 320; // 320-330粉色范围
  const horizontalMovement = Math.random() * 100 - 50; // -50px 到 50px

  return {
    id: generateId(),
    size,
    left,
    top,
    opacity,
    hue,
    duration,
    delay,
    horizontalMovement,
  };
};

// 创建花瓣动画样式
const createPetalAnimation = (petal: Petal): void => {
  const styleSheet = document.createElement("style");
  document.head.appendChild(styleSheet);

  // 修改后的关键帧（去除翻转效果）
  const keyframes = `
  @keyframes fall-${petal.id} {
    0% {
      transform: translate(0, 0) rotate(0deg);
      opacity: ${petal.opacity};
    }
    25% {
      transform: translate(${petal.horizontalMovement * 0.25}px, ${
    maxHeight.value * 0.25
  }px) 
                 rotate(${90 + Math.random() * 90}deg);
      opacity: ${petal.opacity * 0.9};
    }
    50% {
      transform: translate(${petal.horizontalMovement * 0.5}px, ${
    maxHeight.value * 0.5
  }px) 
                 rotate(${180 + Math.random() * 90}deg);
      opacity: ${petal.opacity * 0.7};
    }
    75% {
      transform: translate(${petal.horizontalMovement * 0.75}px, ${
    maxHeight.value * 0.75
  }px) 
                 rotate(${270 + Math.random() * 90}deg);
      opacity: ${petal.opacity * 0.5};
    }
    100% {
      transform: translate(${petal.horizontalMovement}px, ${maxHeight.value}px) 
                 rotate(${360 + Math.random() * 180}deg);
      opacity: 0;
    }
  }
`;

  styleSheet.innerHTML = keyframes;
  petal.styleSheet = styleSheet;
};

// 创建初始花瓣
const createPetals = (count: number): void => {
  for (let i = 0; i < count; i++) {
    const petal = createPetal();
    createPetalAnimation(petal);
    petals.value.push(petal);
  }
};

// 移除所有花瓣
const removeAllPetals = (): void => {
  petals.value.forEach((petal) => {
    if (petal.styleSheet && petal.styleSheet.parentNode) {
      petal.styleSheet.parentNode.removeChild(petal.styleSheet);
    }
  });
  petals.value = [];
};

// 设置最大高度
const setMaxHeight = (): void => {
  if (containerRef.value && containerRef.value.parentElement) {
    maxHeight.value = containerRef.value.parentElement.clientHeight;
  } else {
    maxHeight.value = window.innerHeight;
  }
};

// 重新创建所有花瓣（用于窗口大小变化）
const recreatePetals = (): void => {
  removeAllPetals();
  createPetals(petalCount.value);
};

// 监听强度变化
watch(
  () => props.intensity,
  (newIntensity) => {
    petalCount.value = Math.floor(25 * newIntensity);
    recreatePetals();
  }
);

// 监听启用状态变化
watch(
  () => props.enabled,
  (newVal) => {
    if (newVal) {
      setMaxHeight();
      createPetals(petalCount.value);
    } else {
      removeAllPetals();
    }
  }
);

onMounted(() => {
  nextTick(() => {
    setMaxHeight();
    if (props.enabled) {
      createPetals(petalCount.value);
    }
  });

  window.addEventListener("resize", () => {
    setMaxHeight();
    recreatePetals();
  });
});

onUnmounted(() => {
  removeAllPetals();
  window.removeEventListener("resize", setMaxHeight);
});
</script>

<style scoped>
.petal-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: -1;
  overflow: hidden;
}

.petal {
  position: absolute;
  border-radius: 50% 0 50% 50%;
  opacity: 0.7;
  filter: drop-shadow(0 0 5px rgba(255, 182, 193, 0.5));
  transform-origin: center;
}
</style>
