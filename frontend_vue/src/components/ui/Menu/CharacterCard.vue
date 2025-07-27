<template>
  <div class="character-card">
    <div class="character-avatar-container">
      <img :src="avatar" :alt="name" class="character-avatar" />
    </div>
    <div class="character-content-wrapper">
      <h5 class="character-title">{{ name }}</h5>
      <p class="character-description">{{ info }}</p>
      <Button type="select">✓ 已选择</Button>
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
  background-color: #fff;
  border-radius: 12px;
  box-shadow: 0 4px 15px rgba(0, 0, 0, 0.05);
  overflow: hidden;
  transition: all 0.3s ease;
}

.character-card:hover {
  transform: translateY(-5px);
  box-shadow: 0 8px 25px rgba(0, 0, 0, 0.1);
}

.character-avatar-container {
  width: 150px;
  height: 100%;
  flex-shrink: 0;
  padding: 15px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #f5f7fa 0%, #e8ecf3 100%);
}

.character-avatar {
  width: 100%;
  height: auto;
  object-fit: contain;
  border-radius: 8px;
}

.character-content-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 15px;
  position: relative;
}

.character-title {
  font-size: 16px;
  font-weight: 700;
  color: #333;
  margin-top: 0;
  margin-bottom: 8px;
}

.character-description {
  font-size: 13px;
  color: #666;
  line-height: 1.5;
  flex-grow: 1;
  margin-bottom: 15px;
}
</style>
