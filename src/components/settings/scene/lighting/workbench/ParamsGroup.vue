<script setup lang="ts">
/** 参数分组：标题 + 可选重置按钮 + 内容。 */
defineProps<{
  title: string;
  /** 标题右侧的次要说明（如 Normal & Depth） */
  note?: string;
  resetLabel?: string;
}>();

const emit = defineEmits<{ (e: "reset"): void }>();
</script>

<template>
  <section class="wb-group">
    <header class="wb-group-head">
      <slot name="icon" />
      <span class="wb-group-title">{{ title }}</span>
      <span v-if="note" class="wb-group-note">{{ note }}</span>
      <span class="wb-group-tail">
        <slot name="head-extra" />
        <button
          v-if="resetLabel"
          type="button"
          class="wb-mini-link"
          :title="resetLabel"
          @click="emit('reset')"
        >
          {{ resetLabel }}
        </button>
      </span>
    </header>
    <div class="wb-group-body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.wb-group {
  border-bottom: 1px solid var(--wb-border);
}

.wb-group-head {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 10px;
}

.wb-group-title {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--wb-text);
}

.wb-group-note {
  font-size: 10px;
  color: var(--wb-faint);
}

.wb-group-tail {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 4px;
}

.wb-group-body {
  padding-bottom: 4px;
}

.wb-mini-link {
  font-size: 10.5px;
  color: var(--wb-muted);
  padding: 2px 4px;
  border-radius: 5px;
  transition:
    color 0.12s ease,
    background 0.12s ease;
}

.wb-mini-link:hover {
  color: var(--wb-accent);
  background: var(--wb-accent-weak);
}
</style>
