<template>
  <StaticRolePresentation
    ref="staticPresentationRef"
    :src="src"
    :visible="!ready"
    :layer-style="layerStyle"
    :animation-classes="animationClasses"
    :object-fit="objectFit"
    @animation-end="emit('animation-end')"
  >
    <div
      v-if="unavailable && !src"
      class="absolute inset-0 flex items-center justify-center text-sm text-white/60"
    >
      {{ $t('game.avatar.live2dUnavailable') }}
    </div>
  </StaticRolePresentation>
</template>

<script setup lang="ts">
import { computed, inject, ref } from 'vue'

import { live2dStageContextKey } from '../live2d/live2d-stage-context'
import StaticRolePresentation from './StaticRolePresentation.vue'

const props = defineProps<{
  roleId: number
  src: string
  layerStyle: Record<string, string>
  animationClasses: Record<string, boolean>
  objectFit: string
}>()

const emit = defineEmits<{
  'animation-end': []
}>()

const stage = inject(live2dStageContextKey, null)
const ready = computed(() => stage?.readyRoleIds.value.has(props.roleId) ?? false)
const unavailable = computed(() => stage?.unavailableRoleIds.value.has(props.roleId) ?? false)
const staticPresentationRef = ref<InstanceType<typeof StaticRolePresentation> | null>(null)

async function waitForLoad() {
  await staticPresentationRef.value?.waitForLoad()
}

defineExpose({ waitForLoad })
</script>
