<template>
  <Transition name="slide-up">
    <div v-if="store.isVisible && store.current" class="achievement-toast" :class="typeClass">
      <div class="glow-effect"></div>

      <div class="achievement-icon">
        <img
          v-if="store.current.imgUrl"
          :src="store.current.imgUrl"
          alt="成就图标"
          class="icon-image"
        />
        <div v-else class="default-icon">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="w-8 h-8"
          >
            <path d="M6 9H4.5a2.5 2.5 0 0 1 0-5H6" />
            <path d="M18 9h1.5a2.5 2.5 0 0 0 0-5H18" />
            <path d="M4 22h16" />
            <path d="M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22" />
            <path d="M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20.24 17 22" />
            <path d="M18 2H6v7a6 6 0 0 0 12 0V2Z" />
          </svg>
        </div>
      </div>

      <div class="achievement-content">
        <div class="achievement-header">
          <span class="achievement-label">{{ typeLabel }}</span>
        </div>
        <div class="achievement-title">{{ store.current.title }}</div>
        <div class="achievement-description">{{ store.current.description }}</div>
      </div>

      <div class="progress-bar-container">
        <div
          class="progress-bar"
          :style="{ animationDuration: `${store.current.duration}ms` }"
        ></div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue'
import { useAchievementStore } from '../../stores/modules/ui/achievement'
import { useUIStore } from '../../stores/modules/ui/ui'

const store = useAchievementStore()
const uiStore = useUIStore()

const typeClass = computed(() => {
  return store.current?.type === 'rare' ? 'achievement-gold' : 'achievement-green'
})

const typeLabel = computed(() => {
  return store.current?.type === 'rare' ? '稀有成就' : '成就解锁'
})

// 播放音效逻辑
watch(
  () => store.isVisible,
  (visible) => {
    if (visible && store.current?.audioUrl) {
      const audio = new Audio(store.current.audioUrl)
      audio.volume = (uiStore.achievementVolume ?? 80) / 100
      audio.play().catch((err) => {
        console.warn('成就音效播放失败:', err)
      })
    }
  },
)
</script>

<style scoped>
@reference "tailwindcss";

.achievement-toast {
  @apply fixed bottom-8 right-8 z-[9999];
  @apply flex items-center gap-4;
  @apply p-4 min-w-[320px] max-w-[400px];
  @apply overflow-hidden;
  @apply rounded-xl;

  background: rgba(15, 15, 15, 0.5);
  backdrop-filter: blur(20px);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
  border: 1px solid rgba(255, 255, 255, 0.05);
}

/* --- 绿色 (普通/common) 主题 --- */
.achievement-green {
  border: 1px solid rgba(74, 222, 128, 0.2);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.6),
    0 0 15px rgba(74, 222, 128, 0.1) inset;
}

.achievement-green .glow-effect {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 150%;
  height: 150%;
  background: radial-gradient(circle, rgba(74, 222, 128, 0.1) 0%, transparent 60%);
  z-index: -1;
  filter: blur(20px);
}

.achievement-green .achievement-label {
  @apply text-green-400 text-xs font-bold tracking-wider;
}

.achievement-green .default-icon {
  @apply text-green-400;
  background: rgba(74, 222, 128, 0.1);
}

.achievement-green .progress-bar {
  background-color: #40b681 !important;
}

/* --- 金色 (稀有/rare) 主题 --- */
.achievement-gold {
  background:
    radial-gradient(circle at center, rgba(251, 191, 36, 0.08) 0%, transparent 50%),
    linear-gradient(145deg, rgba(40, 30, 10, 0.7), rgba(10, 10, 10, 0.5));

  border: 1px solid rgba(251, 191, 36, 0.3);
  box-shadow:
    0 10px 40px -10px rgba(0, 0, 0, 0.8),
    0 0 20px rgba(251, 191, 36, 0.2) inset,
    0 0 15px rgba(251, 191, 36, 0.15);
}

.achievement-gold .glow-effect {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 200%;
  height: 200%;
  background: radial-gradient(
    circle,
    rgba(251, 191, 36, 0.25) 0%,
    rgba(251, 191, 36, 0.05) 30%,
    transparent 65%
  );
  z-index: -1;
  filter: blur(30px);
  opacity: 0.8;
}

.achievement-gold .achievement-label {
  @apply text-amber-300 text-xs font-bold tracking-widest uppercase;
  text-shadow: 0 0 8px rgba(251, 191, 36, 0.6);
}

.achievement-gold .achievement-title {
  @apply text-amber-50;
}

.achievement-gold .default-icon {
  @apply text-amber-300;
  background: linear-gradient(135deg, rgba(251, 191, 36, 0.15), rgba(251, 191, 36, 0.02));
  border: 1px solid rgba(251, 191, 36, 0.25);
  box-shadow: 0 0 15px rgba(251, 191, 36, 0.15) inset;
}

.achievement-gold .progress-bar {
  background: linear-gradient(90deg, #fbbf24, #f59e0b, #fbbf24);
  box-shadow: 0 0 10px rgba(251, 191, 36, 0.5);
}

/* 通用布局 */
.achievement-icon {
  @apply shrink-0;
}

.icon-image {
  @apply w-12 h-12 rounded-lg object-cover;
}

.default-icon {
  @apply w-12 h-12 rounded-lg flex items-center justify-center;
}

.achievement-content {
  @apply flex flex-col justify-center gap-0.5 flex-1;
}

.achievement-title {
  @apply text-white font-bold text-sm leading-tight;
}

.achievement-description {
  @apply text-gray-300 text-xs leading-tight;
}

/* 进度条容器 */
.progress-bar-container {
  @apply absolute bottom-0 left-0 w-full h-[2px] bg-gray-800/50;
}

.progress-bar {
  @apply h-full w-full origin-left;
  animation: progress linear forwards;
}

@keyframes progress {
  0% {
    transform: scaleX(1);
  }

  100% {
    transform: scaleX(0);
  }
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.5s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

.slide-up-enter-from,
.slide-up-leave-to {
  transform: translateY(100px) scale(0.9);
  opacity: 0;
}
</style>
