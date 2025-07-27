<template>
  <div class="settings-page active">
    <div class="settings-columns">
      <slot></slot>
    </div>
  </div>
</template>

<script setup lang="ts"></script>

<style scoped>
.settings-page {
  color: white;
  height: 100%;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  padding: 10px 20px;
}

.settings-page.active {
  display: block;
}

/* 为设置内容添加半透明背景，提高可读性 */
.settings-page > * {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 15px;
  margin-bottom: 15px;
}

.settings-columns {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0px;
  height: 100%;
}
</style>
