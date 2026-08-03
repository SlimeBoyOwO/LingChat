<template>
  <div
    :class="{
      [`hidden
      sm:block`]: mobile === false,
    }"
    v-bind="$attrs"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
defineOptions({ inheritAttrs: false })

interface Props {
  mobile?: boolean
}

withDefaults(defineProps<Props>(), {
  mobile: true,
})
</script>
