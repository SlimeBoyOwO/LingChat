<template>
  <Transition name="character-fade">
    <div
      class="role-container-transition pointer-events-none absolute h-full w-full
        origin-[center_0%]"
      :style="layerStyle"
      @animationend="emit('animation-end')"
    >
      <ImageAcrossFade
        v-show="visible"
        ref="imageFadeRef"
        class="absolute h-[102%] w-full"
        :class="animationClasses"
        :src="src"
        :duration="300"
        position="center bottom"
        :object-fit="objectFit"
      />
      <slot></slot>
    </div>
  </Transition>
</template>

<script setup lang="ts">
  import { ref } from "vue";

  import ImageAcrossFade from "@/components/ui/ImageAcrossFade.vue";

  withDefaults(
    defineProps<{
      src: string;
      visible?: boolean;
      layerStyle: Record<string, string>;
      animationClasses: Record<string, boolean>;
      objectFit: string;
    }>(),
    { visible: true }
  );

  const emit = defineEmits<{
    "animation-end": [];
  }>();

  const imageFadeRef = ref<InstanceType<typeof ImageAcrossFade> | null>(null);

  async function waitForLoad() {
    await imageFadeRef.value?.waitForLoad();
  }

  defineExpose({ waitForLoad });
</script>

<style scoped>
  .character-fade-enter-active,
  .character-fade-leave-active {
    transition:
      opacity 0.5s ease-in-out,
      transform 0.5s ease-out;
  }

  .character-fade-enter-from,
  .character-fade-leave-to {
    opacity: 0;
  }
</style>
