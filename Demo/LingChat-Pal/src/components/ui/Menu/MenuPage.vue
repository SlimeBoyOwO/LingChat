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
  display: flex;
  color: white;
  height: 100%;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  padding: 10px 20px;
  justify-content: center; /* 水平居中 */
  align-items: center; /* 垂直居中（可选） */
}

.settings-page.active {
  display: block;
}

/* 为设置内容添加半透明背景，提高可读性 */
.settings-page > * {
  /*background: rgba(255, 255, 255, 0.1);*/
  border-radius: 12px;
  padding: 15px;
  margin-bottom: 15px;
}

.settings-columns {
  margin: 0 auto; /* 确保 flex 容器自身居中 */
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  margin-bottom: 30px;
  gap: 20px;
  max-width: var(--menu-max-width);
  height: 100%;
  max-height: 100%;
}
</style>
