<template>
  <transition appear :css="false" @before-enter="beforeEnter" @enter="enter" @leave="leave">
    <!-- 使用 -mt-[20vh] 让整体位置中间偏上 -->
    <div
      v-if="isVisible"
      class="fixed inset-0 flex flex-col items-center justify-center -mt-[20vh] pointer-events-none z-[1000]"
    >
      <!-- 柔和的雾化背景层 -->
      <div
        class="relative w-full py-12 flex flex-col items-center justify-center bg-slate-900/40 backdrop-blur-2xl border-y border-white/10 shadow-[0_0_25px_rgba(0,0,0,0.8)]"
      >
        <!-- 核心调整：利用 mask-image 让背景和高斯模糊在两侧自然淡出，彻底消除横线切割感 -->
        <div class="absolute inset-0 glass-fade-mask -z-10"></div>

        <!-- 粒子效果 (局限在中间区域) -->
        <div class="absolute inset-0 opacity-60 pointer-events-none -z-10 mask-center">
          <div
            class="absolute top-4 left-[30%] w-1.5 h-1.5 rounded-full bg-white/60 animate-float-slow"
          ></div>
          <div
            class="absolute bottom-6 right-[35%] w-1 h-1 rounded-full bg-brand/60 animate-float"
          ></div>
          <div
            class="absolute top-8 right-[30%] w-2 h-2 rounded-full bg-purple-400/50 animate-float-reverse blur-[1px]"
          ></div>
          <div
            class="absolute bottom-4 left-[40%] w-1 h-1 rounded-full bg-cyan-300/40 animate-float-slow"
          ></div>
        </div>

        <!-- 柔和微光扫射 -->
        <div
          class="absolute -inset-full top-0 h-full w-[200%] -z-10 block transform -skew-x-12 bg-gradient-to-r from-transparent via-white/5 to-transparent animate-shine-fast mask-center"
        ></div>

        <!-- 文字与星光内容区域 -->
        <div class="relative z-10 flex flex-col items-center">
          <!-- 顶部装饰线与副标题 + 星光 -->
          <div class="flex items-center gap-4 mb-3 opacity-90">
            <div class="h-[1px] w-12 bg-gradient-to-r from-transparent to-brand/60"></div>

            <!-- 左侧旋转星光 -->
            <svg
              class="w-4 h-4 text-brand animate-star-spin drop-shadow-[0_0_8px_rgba(var(--color-brand),0.8)]"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path d="M12 1L14.5 8.5L22 11L14.5 13.5L12 21L9.5 13.5L2 11L9.5 8.5L12 1Z" />
            </svg>

            <span
              class="text-sm font-light text-brand tracking-[0.5em] uppercase text-shadow-glow translate-x-[0.25em]"
            >
              Story Clear
            </span>

            <!-- 右侧反向旋转星光 -->
            <svg
              class="w-4 h-4 text-brand animate-star-spin-reverse drop-shadow-[0_0_8px_rgba(var(--color-brand),0.8)]"
              viewBox="0 0 24 24"
              fill="currentColor"
            >
              <path d="M12 1L14.5 8.5L22 11L14.5 13.5L12 21L9.5 13.5L2 11L9.5 8.5L12 1Z" />
            </svg>

            <div class="h-[1px] w-12 bg-gradient-to-l from-transparent to-brand/60"></div>
          </div>

          <!-- 剧本名称主标题 -->
          <h2
            class="text-4xl md:text-5xl font-bold text-white tracking-widest drop-shadow-[0_4px_16px_rgba(0,0,0,0.8)] mb-2"
          >
            {{ completedScriptName }}
          </h2>

          <!-- 底部中文提示 (稍微淡化) -->
          <span class="text-white/50 text-xs tracking-[0.3em] mt-3"> 本次剧本已完成 </span>
        </div>
      </div>
    </div>
  </transition>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useGameStore } from '@/stores/modules/game'

const gameStore = useGameStore()

const isVisible = ref(false)
const completedScriptName = ref('')

// 监听游戏状态中的 runningScript
watch(
  () => gameStore.runningScript,
  (newVal, oldVal) => {
    // 从有剧本变成无剧本时触发
    if (!newVal && oldVal) {
      // 提取剧本名称，根据你的数据结构调整
      completedScriptName.value = oldVal.scriptName || 'Unknown Script'

      isVisible.value = true

      // 缩短展示时间，2.5秒后开始淡出
      setTimeout(() => {
        isVisible.value = false
      }, 2500)
    }
  },
)

// === 动画生命周期钩子 ===

function beforeEnter(el: Element) {
  const element = el as HTMLElement
  element.style.opacity = '0'
  element.style.transform = 'scale(1.05) translateY(10px)'
  element.style.filter = 'blur(8px)'
  // 加快入场速度
  element.style.transition = 'all 0.8s cubic-bezier(0.2, 0.8, 0.2, 1)'
}

function enter(el: Element, done: () => void) {
  const element = el as HTMLElement
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      element.style.opacity = '1'
      element.style.transform = 'scale(1) translateY(0)'
      element.style.filter = 'blur(0px)'
      setTimeout(done, 800)
    })
  })
}

function leave(el: Element, done: () => void) {
  const element = el as HTMLElement
  // 离场更加干脆
  element.style.transition = 'all 0.6s cubic-bezier(0.4, 0, 0.2, 1)'
  element.style.opacity = '0'
  element.style.transform = 'scale(0.98) translateY(-10px)'
  element.style.filter = 'blur(4px)'
  setTimeout(done, 600)
}
</script>

<style scoped>
/* 核心：两端透明渐变遮罩，完美消除横线感 */
.glass-fade-mask {
  -webkit-mask-image: linear-gradient(
    to right,
    transparent 0%,
    black 25%,
    black 75%,
    transparent 100%
  );
  mask-image: linear-gradient(to right, transparent 0%, black 25%, black 75%, transparent 100%);
}

.mask-center {
  -webkit-mask-image: radial-gradient(ellipse at center, black 20%, transparent 70%);
  mask-image: radial-gradient(ellipse at center, black 20%, transparent 70%);
}

/* 悬浮与扫光动画 (复用选项组件风格) */
@keyframes float {
  0%,
  100% {
    transform: translateY(0) translateX(0);
  }
  50% {
    transform: translateY(-4px) translateX(4px);
  }
}
@keyframes float-slow {
  0%,
  100% {
    transform: translateY(0) translateX(0);
  }
  50% {
    transform: translateY(3px) translateX(-3px);
  }
}
@keyframes float-reverse {
  0%,
  100% {
    transform: translateY(0) translateX(0);
  }
  50% {
    transform: translateY(-3px) translateX(-2px);
  }
}
@keyframes shine-fast {
  0% {
    left: -100%;
  }
  20% {
    left: 100%;
  }
  100% {
    left: 100%;
  }
}

/* 星光旋转动画 */
@keyframes star-spin {
  0% {
    transform: rotate(0deg) scale(0.9);
    opacity: 0.7;
  }
  50% {
    transform: rotate(180deg) scale(1.1);
    opacity: 1;
  }
  100% {
    transform: rotate(360deg) scale(0.9);
    opacity: 0.7;
  }
}

@keyframes star-spin-reverse {
  0% {
    transform: rotate(0deg) scale(0.9);
    opacity: 0.7;
  }
  50% {
    transform: rotate(-180deg) scale(1.1);
    opacity: 1;
  }
  100% {
    transform: rotate(-360deg) scale(0.9);
    opacity: 0.7;
  }
}

.animate-float {
  animation: float 5s ease-in-out infinite;
}
.animate-float-slow {
  animation: float-slow 7s ease-in-out infinite;
}
.animate-float-reverse {
  animation: float-reverse 6s ease-in-out infinite;
}
.animate-shine-fast {
  animation: shine-fast 3s ease-in-out infinite;
}

.animate-star-spin {
  animation: star-spin 4s linear infinite;
}
.animate-star-spin-reverse {
  animation: star-spin-reverse 4s linear infinite;
}

.text-shadow-glow {
  text-shadow: 0 0 12px rgba(var(--color-brand), 0.6);
}
</style>
