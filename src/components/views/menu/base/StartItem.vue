<template>
  <button class="start-item" v-bind="$attrs">
    <slot />
  </button>
</template>

<style scoped>
.start-item {
  background: transparent;
  color: white;
  border: none;
  margin-top: 15px;
  padding: 5px;
  font-size: clamp(40px, 4vw, 72px);
  font-family: 'Maoken Assorted Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto,
    'Helvetica Neue', Arial, sans-serif;
  cursor: pointer;
  text-align: justify;
  white-space: nowrap;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
  transition: all 0.3s cubic-bezier(0.18, 0.89, 0.32, 1.28);
}

.start-item:hover:not(:disabled) {
  color: #f0f0f0;
  text-shadow: 0 0 6px rgba(255, 255, 255, 0.4);
  transform: translateY(-8px) scale(1.05);
}

.start-item:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.start-item:hover:disabled {
  color: white;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
}
</style>
