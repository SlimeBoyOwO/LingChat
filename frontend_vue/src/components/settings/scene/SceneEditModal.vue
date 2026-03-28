<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="show"
        class="fixed inset-0 z-[9999] flex items-center justify-center p-4 backdrop-blur-md bg-black/40"
        @click="$emit('close')"
      >
        <div
          class="relative w-full max-w-xl max-h-[85vh] overflow-hidden rounded-3xl border border-white/20 bg-slate-900/40 backdrop-blur-2xl shadow-2xl flex flex-col"
          @click.stop
        >
          <!-- Header -->
          <div class="p-6 flex items-center justify-between bg-white/10 border-b border-white/10">
            <h3 class="text-2xl font-bold text-white leading-none">
              {{ mode === 'create' ? '添加场景' : '更新场景' }}
            </h3>
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
          <div class="flex-1 overflow-y-auto p-8 space-y-6">
            <!-- 场景名称 -->
            <section>
              <label
                class="flex items-center gap-2 text-sm font-bold text-gray-200/90 tracking-widest uppercase mb-3 opacity-80"
              >
                <span class="w-1 h-4 bg-orange-500 rounded-full"></span> 场景名称
              </label>
              <input
                v-model="formData.sceneName"
                placeholder="例如：海边日落"
                class="w-full px-4 py-3 rounded-xl border border-white/10 bg-black/40 text-white placeholder-white/30 focus:border-indigo-400 focus:ring-1 focus:ring-indigo-400/50 focus:outline-none transition-all"
              />
            </section>

            <!-- 场景图片 -->
            <section>
              <label
                class="flex items-center gap-2 text-sm font-bold text-gray-200/90 tracking-widest uppercase mb-3 opacity-80"
              >
                <span class="w-1 h-4 bg-indigo-500 rounded-full"></span> 场景图片
              </label>
              <div class="flex gap-3">
                <select
                  v-model="formData.sceneImage"
                  class="flex-1 px-4 py-3 rounded-xl border border-white/10 bg-black/40 text-white focus:border-indigo-400 focus:ring-1 focus:ring-indigo-400/50 focus:outline-none transition-all appearance-none"
                >
                  <option value="" class="bg-slate-800 text-white/50">选择背景图片</option>
                  <option
                    v-for="bg in backgrounds"
                    :key="bg.url"
                    :value="getFilename(bg.url)"
                    class="bg-slate-800 text-white"
                  >
                    {{ bg.title }}
                  </option>
                </select>
                <Button
                  @click="$emit('upload')"
                  class="!bg-white/10 hover:!bg-white/20 !text-white border border-white/20 whitespace-nowrap px-6 rounded-xl"
                >
                  上传
                </Button>
              </div>
            </section>

            <!-- 场景描述 -->
            <section>
              <label
                class="flex items-center gap-2 text-sm font-bold text-gray-200/90 tracking-widest uppercase mb-3 opacity-80"
              >
                <span class="w-1 h-4 bg-emerald-500 rounded-full"></span> 场景描述
              </label>
              <textarea
                v-model="formData.sceneDescription"
                placeholder="描述场景的环境、氛围、光线等"
                rows="5"
                class="w-full px-4 py-3 rounded-xl border border-white/10 bg-black/40 text-white placeholder-white/30 focus:border-indigo-400 focus:ring-1 focus:ring-indigo-400/50 focus:outline-none transition-all resize-none"
              ></textarea>
            </section>

            <!-- 自动分析开关 -->
            <section class="flex items-center gap-3 pt-2">
              <Toggle
                :checked="formData.autoAnalyze"
                @change="formData.autoAnalyze = $event"
                :disabled="!formData.sceneImage"
              />
              <span class="text-sm font-medium text-white/70">自动分析图片生成描述</span>
            </section>
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
              @click="handleSubmit"
              :disabled="!formData.sceneName.trim()"
              class="!bg-indigo-500 hover:!bg-indigo-400 border-none shadow-[0_0_10px_rgba(99,102,241,0.5)] disabled:opacity-50 disabled:shadow-none min-w-[100px] text-white"
            >
              {{ mode === 'create' ? '创建' : '更新' }}
            </Button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { reactive, watch } from 'vue'
import { Button, Toggle } from '../../base'
import type { BackgroundImageInfo } from '../../../types'

const props = defineProps<{
  show: boolean
  mode: 'create' | 'update'
  backgrounds: BackgroundImageInfo[]
  initialData?: {
    sceneName: string
    sceneImage: string | null
    sceneDescription: string
  }
}>()

const emit = defineEmits<{
  close: []
  submit: [
    data: {
      sceneName: string
      sceneImage: string | null
      sceneDescription: string
      autoAnalyze: boolean
    },
  ]
  upload: []
}>()

const formData = reactive({
  sceneName: '',
  sceneImage: '',
  sceneDescription: '',
  autoAnalyze: false,
})

watch(
  () => props.show,
  (val) => {
    if (val && props.initialData) {
      formData.sceneName = props.initialData.sceneName
      formData.sceneImage = props.initialData.sceneImage || ''
      formData.sceneDescription = props.initialData.sceneDescription
      formData.autoAnalyze = false
    } else if (val) {
      formData.sceneName = ''
      formData.sceneImage = ''
      formData.sceneDescription = ''
      formData.autoAnalyze = false
    }
  },
)

const getFilename = (url: string): string => {
  const match = url.match(/background_file\/(.+)$/)
  return match && match[1] ? decodeURIComponent(match[1]) : ''
}

const handleSubmit = () => {
  emit('submit', {
    sceneName: formData.sceneName.trim(),
    sceneImage: formData.sceneImage || null,
    sceneDescription: formData.sceneDescription.trim(),
    autoAnalyze: formData.autoAnalyze,
  })
}
</script>
