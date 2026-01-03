<template>
  <Transition name="slide-in">
    <div v-if="uiStore.notification.isVisible" class="notification" :class="typeClass">
      <!-- 角色头像区域 -->
      <div class="notification-avatar">
        <img
          v-if="uiStore.notification.avatarUrl"
          :src="uiStore.notification.avatarUrl"
          alt="avatar"
          class="avatar-image"
        />
      </div>

      <!-- 文字内容区域 -->
      <div class="notification-content">
        <div class="notification-title">
          {{ uiStore.notification.title || '[通知标题]' }}
        </div>
        <div class="notification-message">
          {{ uiStore.notification.message || '[通知内容]' }}
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useUIStore } from '../../stores/modules/ui/ui'

const uiStore = useUIStore()

// 根据类型返回对应的CSS类
const typeClass = computed(() => `notification-${uiStore.notification.type}`)
</script>

<style scoped>
@reference "tailwindcss";

.notification {
  @apply fixed top-5 left-0 z-[10000];
  @apply flex items-center gap-4;
  @apply px-6 py-4 min-w-80 max-w-[480px];

  /* 玻璃态效果 */
  background: linear-gradient(135deg, rgba(30, 30, 40, 0.95) 0%, rgba(20, 20, 30, 0.9) 100%);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);

  border-radius: 0 16px 16px 0;
  box-shadow:
    4px 4px 20px rgba(0, 0, 0, 0.4),
    inset 0 1px 0 rgba(255, 255, 255, 0.1);
}

/* 类型样式 */
.notification-error {
  box-shadow:
    0 0 15px rgba(255, 100, 100, 0.5),
    0 0 30px rgba(255, 100, 100, 0.3),
    0 0 45px rgba(255, 100, 100, 0.15),
    inset 0 0 20px rgba(255, 100, 100, 0.1);
}

.notification-success {
  box-shadow:
    0 0 15px rgba(100, 255, 150, 0.5),
    0 0 30px rgba(100, 255, 150, 0.3),
    0 0 45px rgba(100, 255, 150, 0.15),
    inset 0 0 20px rgba(100, 255, 150, 0.1);
}

.notification-info {
  box-shadow:
    0 0 15px rgba(100, 180, 255, 0.5),
    0 0 30px rgba(100, 180, 255, 0.3),
    0 0 45px rgba(100, 180, 255, 0.15),
    inset 0 0 20px rgba(100, 180, 255, 0.1);
}

.notification-warning {
  box-shadow:
    0 0 15px rgba(255, 200, 100, 0.5),
    0 0 30px rgba(255, 200, 100, 0.3),
    0 0 45px rgba(255, 200, 100, 0.15),
    inset 0 0 20px rgba(255, 200, 100, 0.1);
}

.notification-avatar {
  @apply shrink-0 w-16 h-16 rounded-full overflow-hidden;
  @apply flex items-center justify-center;
  background: rgba(255, 255, 255, 0.1);
}

.avatar-image {
  @apply w-full h-full object-cover;
}

.notification-content {
  @apply flex-1 flex flex-col gap-1.5;
}

.notification-title {
  @apply text-base font-semibold;
  color: rgba(255, 255, 255, 0.95);
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.notification-error .notification-title {
  color: rgba(255, 150, 150, 1);
}

.notification-success .notification-title {
  color: rgba(150, 255, 180, 1);
}

.notification-info .notification-title {
  color: rgba(150, 200, 255, 1);
}

.notification-warning .notification-title {
  color: rgba(255, 220, 150, 1);
}

.notification-message {
  @apply text-sm leading-relaxed;
  color: rgba(255, 255, 255, 0.8);
}

/* 动画 */
.slide-in-enter-active,
.slide-in-leave-active {
  transition:
    transform 0.4s cubic-bezier(0.16, 1, 0.3, 1),
    opacity 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}

.slide-in-enter-from {
  transform: translateX(-100%);
  opacity: 0;
}

.slide-in-leave-to {
  transform: translateX(-100%);
  opacity: 0;
}

/* 响应式 */
@media (max-width: 520px) {
  .notification {
    max-width: calc(100vw - 20px);
    min-width: 280px;
  }
}
</style>
