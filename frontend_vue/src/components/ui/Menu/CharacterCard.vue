<template>
  <div class="character-card">
    <div class="character-avatar-container">
      <img :src="avatar" :alt="name" class="character-avatar" />
    </div>
    <div class="character-content">
      <h5 class="character-title">{{ name }}</h5>
      <p class="character-description">{{ info }}</p>

      <div class="character-actions">
        <slot name="actions"></slot>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import Button from "../../base/widget/Button.vue";

interface CharacterProps {
  avatar?: string;
  name?: string;
  info?: string;
}

const props = withDefaults(defineProps<CharacterProps>(), {});
</script>

<style scoped>
.character-card {
  display: flex;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px) saturate(180%);
  border-radius: 16px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  overflow: hidden;
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  border: 1px solid rgba(0, 0, 0, 0.05);
  height: 180px; /* 固定高度保持统一 */
}

.character-card:hover {
  transform: translateY(-3px);
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12);
}

.character-avatar-container {
  width: 180px;
  height: 180px;
  padding: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: #f8f9fa;
  border-right: 1px solid rgba(0, 0, 0, 0.05);
  border-radius: 16px;
}

.character-avatar {
  width: 180px;
  height: 180px;
  object-fit: contain; /* 保持原始比例，完整显示 */
  border-radius: 8px;
  padding: 5px;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.05);
}

.character-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 12px;
  position: relative;
}

.character-title {
  font-size: 18px;
  font-weight: 600;
  color: #ffffff;
  margin-bottom: 8px;
  font-family: "Segoe UI", Tahoma, Geneva, Verdana, sans-serif;
}

.character-description {
  font-size: 13px;
  color: #f8f9fa;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin-bottom: 15px;
  height: 100px;
}

/* 响应式调整 */
@media (max-width: 768px) {
  .character-card {
    height: 120px;
  }

  .character-avatar-container {
    width: 100px;
    height: 100px;
  }

  .character-avatar {
    width: 85px;
    height: 85px;
  }

  .character-title {
    font-size: 16px;
  }

  .character-description {
    font-size: 12px;
  }

  .character-select-btn {
    padding: 5px 10px;
    font-size: 12px;
  }
}
</style>
