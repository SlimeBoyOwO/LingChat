<template>
  <div class="snow-container" ref="containerRef">
    <div
      class="snowflake"
      v-for="(snowflake, index) in snowflakes"
      :key="index"
      :style="{
        width: `${snowflake.size}px`,
        height: `${snowflake.size}px`,
        left: `${snowflake.left}px`,
        top: `${snowflake.top}px`,
        opacity: snowflake.opacity,
        animation: `fall-${snowflake.id} ${snowflake.duration}s linear ${snowflake.delay}s infinite`,
      }"
    >
      {{ snowflake.content }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from "vue";

// 雪花接口定义
interface Snowflake {
  id: string;
  size: number;
  left: number;
  top: number;
  opacity: number;
  duration: number;
  delay: number;
  horizontalMovement: number;
  content: string;
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
const snowflakes = ref<Snowflake[]>([]);
const containerRef = ref<HTMLElement | null>(null);
const maxHeight = ref(0);

// 雪花数量根据强度调整
const snowflakeCount = ref(Math.floor(50 * props.intensity));

// 雪花字符
const snowflakeChars = ["❄", "❅", "❆", "•", "·"];

// 生成随机ID
const generateId = (): string => {
  return Math.random().toString(36).substr(2, 9);
};

// 创建单个雪花
const createSnowflake = (): Snowflake => {
  const size = Math.random() * 10 + 8; // 8-18px
  const left = Math.random() * window.innerWidth;
  const top = -30;
  const duration = Math.random() * 15 + 20; // 20-35秒
  const delay = Math.random() * 10; // 0-10秒延迟
  const opacity = Math.random() * 0.7 + 0.3; // 0.3-1.0透明度
  const horizontalMovement = Math.random() * 100 - 50; // -50px 到 50px
  const content =
    snowflakeChars[Math.floor(Math.random() * snowflakeChars.length)];

  return {
    id: generateId(),
    size,
    left,
    top,
    opacity,
    duration,
    delay,
    horizontalMovement,
    content,
  };
};

// 创建雪花动画样式
const createSnowflakeAnimation = (snowflake: Snowflake): void => {
  const styleSheet = document.createElement("style");
  document.head.appendChild(styleSheet);

  // 雪花飘落关键帧
  const keyframes = `
  @keyframes fall-${snowflake.id} {
    0% {
      transform: translate(0, 0) rotate(0deg);
      opacity: ${snowflake.opacity};
    }
    25% {
      transform: translate(${snowflake.horizontalMovement * 0.25}px, ${
    maxHeight.value * 0.25
  }px) 
                 rotate(${Math.random() * 90}deg);
      opacity: ${snowflake.opacity * 0.95};
    }
    50% {
      transform: translate(${snowflake.horizontalMovement * 0.5}px, ${
    maxHeight.value * 0.5
  }px) 
                 rotate(${Math.random() * 180}deg);
      opacity: ${snowflake.opacity * 0.9};
    }
    75% {
      transform: translate(${snowflake.horizontalMovement * 0.75}px, ${
    maxHeight.value * 0.75
  }px) 
                 rotate(${Math.random() * 270}deg);
      opacity: ${snowflake.opacity * 0.8};
    }
    100% {
      transform: translate(${snowflake.horizontalMovement}px, ${
    maxHeight.value
  }px) 
                 rotate(${Math.random() * 360}deg);
      opacity: ${snowflake.opacity * 0.4};;
    }
  }
`;

  styleSheet.innerHTML = keyframes;
  snowflake.styleSheet = styleSheet;
};

// 创建初始雪花
const createSnowflakes = (count: number): void => {
  for (let i = 0; i < count; i++) {
    const snowflake = createSnowflake();
    createSnowflakeAnimation(snowflake);
    snowflakes.value.push(snowflake);
  }
};

// 移除所有雪花
const removeAllSnowflakes = (): void => {
  snowflakes.value.forEach((snowflake) => {
    if (snowflake.styleSheet && snowflake.styleSheet.parentNode) {
      snowflake.styleSheet.parentNode.removeChild(snowflake.styleSheet);
    }
  });
  snowflakes.value = [];
};

// 设置最大高度
const setMaxHeight = (): void => {
  if (containerRef.value && containerRef.value.parentElement) {
    maxHeight.value = containerRef.value.parentElement.clientHeight;
  } else {
    maxHeight.value = window.innerHeight;
  }
};

// 重新创建所有雪花（用于窗口大小变化）
const recreateSnowflakes = (): void => {
  removeAllSnowflakes();
  createSnowflakes(snowflakeCount.value);
};

// 监听强度变化
watch(
  () => props.intensity,
  (newIntensity) => {
    snowflakeCount.value = Math.floor(50 * newIntensity);
    recreateSnowflakes();
  }
);

// 监听启用状态变化
watch(
  () => props.enabled,
  (newVal) => {
    if (newVal) {
      setMaxHeight();
      createSnowflakes(snowflakeCount.value);
    } else {
      removeAllSnowflakes();
    }
  }
);

onMounted(() => {
  nextTick(() => {
    setMaxHeight();
    if (props.enabled) {
      createSnowflakes(snowflakeCount.value);
    }
  });

  window.addEventListener("resize", () => {
    setMaxHeight();
    recreateSnowflakes();
  });
});

onUnmounted(() => {
  removeAllSnowflakes();
  window.removeEventListener("resize", setMaxHeight);
});
</script>

<style scoped>
.snow-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 9999;
  overflow: hidden;
}

.snowflake {
  position: absolute;
  color: white;
  text-align: center;
  user-select: none;
  pointer-events: none;
  text-shadow: 0 0 5px rgba(255, 255, 255, 0.5);
  opacity: 0.7;
  transform-origin: center;
}
</style>
