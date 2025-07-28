<template>
  <section>
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
});
</script>

<style scoped>
section {
  margin-bottom: 25px;
  width: 100%;
  max-width: 900px;
}

div {
  width: 100%;
  max-width: 900px;
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
}

slot {
  display: flex;
}
</style>
