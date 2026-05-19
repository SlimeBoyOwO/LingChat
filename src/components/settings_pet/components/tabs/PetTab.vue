<template>
  <article class="w-full flex-1 flex flex-col">
    <header
      class="mb-6 flex items-end justify-between border-b-2 pb-2 transition-colors"
      :class="isDarkMode ? 'border-slate-700' : 'border-slate-100'"
    >
      <div>
        <h2
          class="text-xl font-black tracking-wide mb-1 flex items-center gap-2 transition-colors"
          :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'"
        >
          桌宠体型设置
        </h2>
        <p
          class="text-xs font-medium transition-colors"
          :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
        >
          调整助手主体、输入框与对话框的缩放比例
        </p>
      </div>
      <span
        class="text-4xl font-bold italic select-none font-mono transition-colors"
        :class="isDarkMode ? 'text-slate-700' : 'text-sky-100'"
        >01</span
      >
    </header>

    <div class="grid grid-cols-2 gap-4 mt-4">
      <div
        @click="selectMode('normal')"
        class="rounded-xl border p-5 shadow-sm relative overflow-hidden cursor-pointer transition-all duration-300 hover:-translate-y-1 min-h-35 flex flex-col"
        :class="[
          isDarkMode
            ? 'border-slate-700 hover:border-sky-500 hover:bg-slate-800'
            : 'border-slate-200 hover:border-sky-400 hover:bg-slate-50',
          currentMode === 'normal'
            ? isDarkMode
              ? 'bg-slate-800 ring-2 ring-sky-500'
              : 'bg-sky-50 ring-2 ring-sky-400'
            : isDarkMode
              ? 'bg-slate-800/50'
              : 'bg-white',
        ]"
      >
        <div class="flex items-center gap-3 mb-2">
          <div
            :class="[
              'p-2 rounded-lg transition-colors',
              currentMode === 'normal' || !isDarkMode
                ? 'bg-sky-100/10 text-sky-500'
                : 'bg-slate-800 text-slate-400',
            ]"
          >
            <MessageSquare class="w-5 h-5" />
          </div>
          <h3 class="font-bold text-lg" :class="isDarkMode ? 'text-slate-200' : 'text-slate-800'">
            陪伴模式
          </h3>
        </div>
        <p class="text-xs" :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'">
          正常的桌宠陪伴模式捏
        </p>
      </div>

      <div
        @click="selectMode('game')"
        class="rounded-xl border p-5 shadow-sm relative overflow-hidden cursor-pointer transition-all duration-300 hover:-translate-y-1 min-h-35 flex flex-col"
        :class="[
          isDarkMode
            ? 'border-slate-700 hover:border-violet-500 hover:bg-slate-800'
            : 'border-slate-200 hover:border-violet-400 hover:bg-slate-50',
          currentMode === 'game'
            ? isDarkMode
              ? 'bg-slate-800 ring-2 ring-violet-500'
              : 'bg-violet-50 ring-2 ring-violet-400'
            : isDarkMode
              ? 'bg-slate-800/50'
              : 'bg-white',
        ]"
      >
        <div class="flex items-center gap-3 mb-2">
          <div
            :class="[
              'p-2 rounded-lg transition-colors',
              currentMode === 'game' || !isDarkMode
                ? 'bg-violet-100/10 text-violet-500'
                : 'bg-slate-800 text-slate-400',
            ]"
          >
            <Gamepad2 class="w-5 h-5" />
          </div>
          <h3 class="font-bold text-lg" :class="isDarkMode ? 'text-slate-200' : 'text-slate-800'">
            游戏模式
          </h3>
        </div>
        <p class="text-xs" :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'">
          可以让ta更多的感知你的屏幕
        </p>
      </div>
    </div>

    <div
      class="rounded-xl border mt-4 p-6 shadow-sm relative overflow-hidden group transition-colors duration-300"
      :class="isDarkMode ? 'bg-slate-800/50 border-slate-700' : 'bg-white border-slate-200'"
    >
      <Ruler
        class="absolute -bottom-4 -right-4 w-32 h-32 opacity-50 -rotate-12 transition-all duration-300 group-hover:scale-110"
        :class="isDarkMode ? 'text-slate-700' : 'text-slate-50'"
      />

      <div class="relative z-10">
        <h3
          class="font-bold text-lg mb-4 flex items-center gap-2"
          :class="isDarkMode ? 'text-slate-200' : 'text-slate-800'"
        >
          <Volume2 class="w-5 h-5 text-sky-500" />
          桌宠大小设置
        </h3>
        <div class="flex items-end gap-3 mb-6">
          <div class="text-4xl font-bold text-sky-500 tracking-tighter">
            {{ percentLabel }}
          </div>
          <div
            class="text-[10px] font-mono font-bold mb-1.5 px-2 py-0.5 rounded uppercase transition-colors"
            :class="isDarkMode ? 'bg-slate-700 text-slate-400' : 'bg-slate-100 text-slate-500'"
          >
            CURRENT SCALE
          </div>
        </div>

        <div class="mt-8 mb-6 relative">
          <input
            type="range"
            :min="PET_SCALE_MIN"
            :max="PET_SCALE_MAX"
            :step="0.01"
            :value="petScale"
            @input="onScaleInput"
            class="custom-slider"
          />
          <div
            class="flex justify-between text-[11px] mt-4 font-mono font-bold transition-colors"
            :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
          >
            <span>MIN {{ PET_SCALE_MIN * 100 }}%</span>
            <span
              class="text-sky-500 relative pl-2 before:content-[''] before:absolute before:left-0 before:top-1.5 before:w-1 before:h-1 before:bg-sky-400 before:rounded-full"
              >DEF 100%</span
            >
            <span>MAX {{ PET_SCALE_MAX * 100 }}%</span>
          </div>
        </div>

        <div
          class="flex justify-end pt-4 border-t mt-6 transition-colors"
          :class="isDarkMode ? 'border-slate-700' : 'border-slate-100/80'"
        >
          <button
            type="button"
            @click="$emit('resetScale')"
            class="px-5 py-2 bg-sky-500 text-white font-bold text-[13px] rounded-lg transition-all hover:bg-sky-400 active:scale-95 flex items-center gap-2 shadow-[0_4px_12px_rgba(56,189,248,0.25)] hover:shadow-[0_6px_16px_rgba(56,189,248,0.35)]"
          >
            <RotateCcw class="w-4 h-4" />
            恢复默认尺寸
          </button>
        </div>
      </div>
    </div>

    <div
      class="rounded-xl border p-6 shadow-sm relative overflow-hidden group transition-colors duration-300 mt-4"
      :class="isDarkMode ? 'bg-slate-800/50 border-slate-700' : 'bg-white border-slate-200'"
    >
      <Sparkles
        class="absolute -bottom-4 -right-4 w-32 h-32 opacity-10 -rotate-12 transition-all duration-300 group-hover:scale-110"
        :class="isDarkMode ? 'text-slate-700' : 'text-slate-300'"
      />

      <div class="relative z-10">
        <h3
          class="font-bold text-lg mb-4 flex items-center gap-2"
          :class="isDarkMode ? 'text-slate-200' : 'text-slate-800'"
        >
          <Sparkles class="w-5 h-5 text-sky-500" />
          背景粒子效果 (无粒子更流畅)
        </h3>

        <div class="flex gap-3">
          <button
            v-for="opt in particleOptions"
            :key="opt.value"
            @click="selectParticle(opt.value)"
            class="flex-1 py-2 px-4 rounded-lg border font-medium text-sm transition-all duration-200 flex items-center justify-center gap-2"
            :class="[
              currentParticle === opt.value
                ? isDarkMode
                  ? 'bg-sky-500/20 border-sky-500 text-sky-400'
                  : 'bg-sky-500 text-white border-sky-500 shadow-md'
                : isDarkMode
                  ? 'bg-transparent border-slate-600 text-slate-400 hover:border-slate-500 hover:text-slate-300'
                  : 'bg-slate-50 border-slate-200 text-slate-600 hover:border-slate-300 hover:bg-slate-100',
            ]"
          >
            <component :is="opt.icon" class="w-4 h-4" v-if="opt.icon" />
            {{ opt.label }}
          </button>
        </div>
      </div>
    </div>

    <div
      class="rounded-xl border mt-4 p-6 shadow-sm relative overflow-hidden group transition-colors duration-300"
      :class="isDarkMode ? 'bg-slate-800/50 border-slate-700' : 'bg-white border-slate-200'"
    >
      <Volume2
        class="absolute -bottom-4 -right-4 w-32 h-32 opacity-10 -rotate-12 transition-all duration-300 group-hover:scale-110"
        :class="isDarkMode ? 'text-slate-700' : 'text-slate-200'"
      />

      <div class="relative z-10">
        <h3
          class="font-bold text-lg mb-4 flex items-center gap-2"
          :class="isDarkMode ? 'text-slate-200' : 'text-slate-800'"
        >
          <Volume2 class="w-5 h-5 text-sky-500" />
          桌宠音量大小
        </h3>
        <div class="flex items-end gap-3 mb-6">
          <div class="text-4xl font-bold text-sky-500 tracking-tighter">
            {{ volumeLabel }}
          </div>
          <div
            class="text-[10px] font-mono font-bold mb-1.5 px-2 py-0.5 rounded uppercase transition-colors"
            :class="isDarkMode ? 'bg-slate-700 text-slate-400' : 'bg-slate-100 text-slate-500'"
          >
            PET VOLUME
          </div>
        </div>

        <div class="mt-8 mb-6 relative">
          <input
            type="range"
            min="0"
            max="100"
            step="1"
            :value="petVolume"
            @input="onVolumeInput"
            class="custom-slider"
          />
          <div
            class="flex justify-between text-[11px] mt-4 font-mono font-bold transition-colors"
            :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
          >
            <span>MIN 0%</span>
            <span
              class="text-sky-500 relative pl-2 before:content-[''] before:absolute before:left-0 before:top-1.5 before:w-1 before:h-1 before:bg-sky-400 before:rounded-full"
              >DEF 50%</span
            >
            <span>MAX 100%</span>
          </div>
        </div>

        <div
          class="flex justify-end pt-4 border-t mt-6 transition-colors"
          :class="isDarkMode ? 'border-slate-700' : 'border-slate-100/80'"
        >
          <button
            type="button"
            @click="$emit('resetVolume')"
            class="px-5 py-2 bg-sky-500 text-white font-bold text-[13px] rounded-lg transition-all hover:bg-sky-400 active:scale-95 flex items-center gap-2 shadow-[0_4px_12px_rgba(56,189,248,0.25)] hover:shadow-[0_6px_16px_rgba(56,189,248,0.35)]"
          >
            <RotateCcw class="w-4 h-4" />
            恢复默认音量
          </button>
        </div>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  Ruler,
  RotateCcw,
  MessageSquare,
  Gamepad2,
  Sparkles,
  Ban,
  Stars,
  Sun,
  Volume2,
} from 'lucide-vue-next'
import { useUIStore } from '../../../../stores/modules/ui/ui'
import { getCurrentWindow } from '@tauri-apps/api/window'

const props = defineProps<{
  isDarkMode: boolean
  petScale: number
  petVolume: number
  PET_SCALE_MIN: number
  PET_SCALE_MAX: number
}>()

const emit = defineEmits<{
  updateScale: [value: number]
  resetScale: []
  updateVolume: [value: number]
  resetVolume: []
}>()

const uiStore = useUIStore()

const currentMode = ref('normal')
const selectMode = (mode: string) => {
  currentMode.value = mode
  // 空函数，留作后续逻辑实现
}

const currentParticle = computed(() => uiStore.currentBackgroundEffect)

const particleOptions = [
  { label: '无', value: 'None', icon: Ban },
  { label: '星空', value: 'StarField', icon: Stars },
  { label: '星辉', value: 'BA', icon: Sun },
]

const selectParticle = async (value: string) => {
  uiStore.setBackgroundEffect(value)
  const appWindow = getCurrentWindow()
  await appWindow.emit('background-effect-changed', { effect: value })
}

const percentLabel = computed(() => {
  return `${Math.round(props.petScale * 100)}%`
})

const volumeLabel = computed(() => {
  return `${Math.round(props.petVolume)}%`
})

const onScaleInput = (event: Event) => {
  const target = event.target as HTMLInputElement
  emit('updateScale', Number(target.value))
}

const onVolumeInput = (event: Event) => {
  const target = event.target as HTMLInputElement
  emit('updateVolume', Number(target.value))
}
</script>

<style scoped>
/* 自定义滑块轨道与手柄 (BA 科技感风格) */
.custom-slider {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  background: transparent;
  outline: none;
}

.custom-slider::-webkit-slider-runnable-track {
  width: 100%;
  height: 6px;
  cursor: pointer;
  background-color: #e2e8f0; /* slate-200 */
  border-radius: 9999px;
  transition: background-color 0.2s;
}

.dark .custom-slider::-webkit-slider-runnable-track {
  background-color: #334155; /* slate-700 */
}

.custom-slider:hover::-webkit-slider-runnable-track {
  background-color: #cbd5e1; /* slate-300 */
}

.dark .custom-slider:hover::-webkit-slider-runnable-track {
  background-color: #475569; /* slate-600 */
}

.custom-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  height: 20px;
  width: 12px;
  border-radius: 4px;
  background-color: #ffffff;
  border: 3px solid #38bdf8; /* sky-400 */
  cursor: pointer;
  margin-top: -7px;
  box-shadow: 0 2px 6px rgba(56, 189, 248, 0.4);
  transition:
    transform 0.1s ease,
    box-shadow 0.1s ease,
    background-color 0.3s ease;
}

.dark .custom-slider::-webkit-slider-thumb {
  background-color: #0f172a; /* slate-900 */
  border: 3px solid #0ea5e9; /* sky-500 */
  box-shadow: 0 2px 6px rgba(14, 165, 233, 0.4);
}

.custom-slider::-webkit-slider-thumb:active {
  transform: scale(0.85);
  box-shadow: 0 1px 3px rgba(56, 189, 248, 0.5);
  border-color: #0ea5e9; /* sky-500 */
}

.dark .custom-slider::-webkit-slider-thumb:active {
  border-color: #38bdf8; /* sky-400 */
}
</style>
