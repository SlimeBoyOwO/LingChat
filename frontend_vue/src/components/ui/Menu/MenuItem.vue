<template>
  <section :class="['menu-item', size]">
    <div class="w-full flex items-center pb-2 mb-4 border-b-2 border-brand space-x-2">
      <slot name="header"></slot>
      <div class="title-wrapper">
        <h4>{{ title }}</h4>
      </div>
    </div>
    <div class="content">
      <slot></slot>
    </div>
  </section>
</template>

<script setup lang="ts">
const props = defineProps({
  title: {
    type: String,
    default: '默认标题',
  },
  size: {
    type: String as () => 'small' | 'large',
    default: 'large',
    validator: (value: string) => ['small', 'large'].includes(value),
  },
})
</script>

<style scoped>
section {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 15px;
}

section.large {
  width: 100%;
}

section.small {
  width: calc(50% - 12.5px); /* 减去一半的margin */
}

.title-wrapper h4 {
  margin: 0;
  font-size: 18px;
  color: #fff;
  font-weight: 600;
  text-shadow: 0 0 2px rgba(0, 0, 0, 0.5);
}

.content {
  width: 100%;
}

/* 响应式设计 - 在小屏幕上让small菜单项变为全宽 */
@media (max-width: 768px) {
  section.small {
    width: 100%;
    max-width: 900px;
  }
}
</style>
