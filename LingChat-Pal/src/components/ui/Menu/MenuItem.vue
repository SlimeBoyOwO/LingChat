<template>
  <section :class="['menu-item', size]">
    <div>
      <h4>{{ title }}</h4>
    </div>
    <slot></slot>
  </section>
</template>

<script setup lang="ts">
const props = defineProps({
  title: {
    type: String,
    default: "默认标题",
  },
  size: {
    type: String as () => "small" | "large",
    default: "large",
    validator: (value: string) => ["small", "large"].includes(value),
  },
});
</script>

<style scoped>
section {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 15px;
}

section.large {
  width: 100%;
  max-width: var(--menu-max-width);
}

section.small {
  width: calc(50% - 12.5px); /* 减去一半的margin */
  max-width: var(--menu-max-width-half);
}

div {
  width: 100%;
  display: flex;
  align-items: center;
  border-bottom: 2px solid var(--accent-color);
  padding-bottom: 8px;
  margin-bottom: 15px;
}

h4 {
  margin: 0;
  font-size: 18px;
  color: #fff;
  font-weight: 600;
  text-shadow: 0 0 2px rgba(0, 0, 0, 0.5);
}

slot {
  display: flex;
}

/* 响应式设计 - 在小屏幕上让small菜单项变为全宽 */
@media (max-width: 768px) {
  section.small {
    width: 100%;
    max-width: 900px;
  }
}
</style>
