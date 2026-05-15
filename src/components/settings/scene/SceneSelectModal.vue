<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="show"
        class="fixed inset-0 z-[9999] flex items-center justify-center p-4 backdrop-blur-md bg-black/40"
        @click="$emit('close')"
      >
        <div
          class="relative w-full max-w-4xl max-h-[85vh] overflow-hidden rounded-3xl border border-white/20 bg-slate-900/40 backdrop-blur-2xl shadow-2xl flex flex-col"
          @click.stop
        >
          <!-- Header -->
          <div class="p-6 flex items-center justify-between bg-white/10 border-b border-white/10">
            <h3 class="text-2xl font-bold text-white leading-none">选择场景</h3>
            <button
              @click="$emit('close')"
              class="p-2 hover:bg-red-500/20 text-white/50 hover:text-white rounded-full transition-colors flex items-center justify-center"
            >
              <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          </div>

          <!-- Body -->
          <div class="flex-1 overflow-y-auto p-6">
            <div class="grid grid-cols-3 gap-6">
              <div
                v-for="scene in scenes"
                :key="scene.id"
                class="group relative rounded-2xl overflow-hidden cursor-pointer border-2 transition-all bg-white/5"
                :class="[
                  selectedId === scene.id
                    ? 'border-indigo-400 shadow-[0_0_15px_rgba(129,140,248,0.5)]'
                    : 'border-white/10 hover:border-white/30 hover:bg-white/10',
                ]"
                @click="selectedId = scene.id"
              >
                <!-- Image Wrapper -->
                <div class="w-full aspect-video bg-black/40 overflow-hidden relative">
                  <img
                    v-if="scene.imageUrl"
                    :src="scene.imageUrl"
                    :alt="scene.sceneName"
                    class="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
                  />
                  <div v-else class="w-full h-full flex items-center justify-center text-white/20">
                    <Image :size="48" />
                  </div>

                  <!-- Selected Checkmark -->
                  <div
                    v-if="selectedId === scene.id"
                    class="absolute top-2 right-2 bg-indigo-500 rounded-full p-1 shadow-lg"
                  >
                    <svg
                      class="w-4 h-4 text-white"
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="3"
                        d="M5 13l4 4L19 7"
                      />
                    </svg>
                  </div>
                </div>

                <!-- Info -->
                <div class="p-4">
                  <div class="font-bold text-sm text-white mb-1 tracking-wider truncate">
                    {{ scene.sceneName }}
                  </div>
                  <div class="text-xs text-white/50 line-clamp-2">{{ scene.sceneDescription }}</div>
                </div>
              </div>
            </div>
          </div>

          <!-- Footer -->
          <div class="p-6 bg-white/5 border-t border-white/10 flex gap-3 justify-end items-center">
            <Button
              @click="$emit('close')"
              class="!bg-transparent !text-white/70 hover:!text-white hover:!bg-white/10 border border-white/20"
            >
              取消
            </Button>
            <Button
              @click="handleConfirm"
              :disabled="!selectedId"
              class="!bg-indigo-500 hover:!bg-indigo-400 border-none shadow-[0_0_10px_rgba(99,102,241,0.5)] disabled:opacity-50 disabled:shadow-none text-white"
            >
              确定
            </Button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { Image } from 'lucide-vue-next'
import { Button } from '../../base'
import type { SceneInfo } from '../../../api/services/scene'

const props = defineProps<{
  show: boolean
  scenes: SceneInfo[]
}>()

const emit = defineEmits<{
  close: []
  confirm: [sceneId: string]
}>()

const selectedId = ref<string>('')

watch(
  () => props.show,
  (val) => {
    if (!val) selectedId.value = ''
  },
)

const handleConfirm = () => {
  if (selectedId.value) {
    emit('confirm', selectedId.value)
  }
}
</script>
