<template>
  <Transition name="slide-in">
    <div v-if="isVisible" class="notification" :class="typeClass">
      <!-- 角色头像区域 -->
      <div class="notification-avatar">
        <img 
          v-if="avatarUrl" 
          :src="avatarUrl" 
          alt="avatar"
          class="avatar-image"
        />
      </div>
      
      <!-- 文字内容区域 -->
      <div class="notification-content">
        <div class="notification-title">
          {{ title || '[通知标题]' }}
        </div>
        <div class="notification-message">
          {{ message || '[通知内容]' }}
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import type { NotificationType } from '../../composables/ui/useNotification';

interface Props {
  type?: NotificationType;
  title?: string;
  message?: string;
  avatarUrl?: string;
  duration?: number;
  isVisible?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  type: 'info',
  title: '',
  message: '',
  avatarUrl: '',
  duration: 3000,
  isVisible: false,
});

const isVisible = ref(props.isVisible);

// 根据类型返回对应的CSS类
const typeClass = computed(() => `notification-${props.type}`);

// 监听外部传入的 isVisible 变化
watch(() => props.isVisible, (newVal) => {
  isVisible.value = newVal;
});
</script>

<style scoped>
.notification {
  position: fixed;
  top: 20px;
  left: 0;
  z-index: 10000;
  
  display: flex;
  align-items: center;
  gap: 16px;
  
  padding: 16px 24px;
  min-width: 320px;
  max-width: 480px;
  
  /* 玻璃态效果 */
  background: linear-gradient(135deg, 
    rgba(30, 30, 40, 0.95) 0%,
    rgba(20, 20, 30, 0.90) 100%
  );
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
  flex-shrink: 0;
  width: 64px;
  height: 64px;
  border-radius: 50%;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.1);
  display: flex;
  align-items: center;
  justify-content: center;
}

.avatar-image {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.notification-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.notification-title {
  font-size: 16px;
  font-weight: 600;
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
  font-size: 14px;
  color: rgba(255, 255, 255, 0.8);
  line-height: 1.4;
}

/* 动画 */
.slide-in-enter-active,
.slide-in-leave-active {
  transition: transform 0.4s cubic-bezier(0.16, 1, 0.3, 1), 
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
