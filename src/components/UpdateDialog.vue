<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 scale-95"
      enter-to-class="opacity-100 scale-100"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100 scale-100"
      leave-to-class="opacity-0 scale-95"
    >
      <div v-if="visible" class="fixed inset-0 z-10000 flex items-center justify-center p-4">
        <!-- 背景遮罩 -->
        <div class="absolute inset-0 bg-slate-900/60 backdrop-blur-sm"></div>

        <!-- 对话框主体 -->
        <div class="relative z-10 bg-white w-full max-w-lg rounded-[2.5rem] shadow-2xl p-8">
          <!-- 标题 -->
          <div class="text-center mb-4">
            <h3 class="text-xl font-black text-slate-800 tracking-tight">
              {{ dialogTitle }}
            </h3>
          </div>

          <!-- 内容区域 -->
          <div class="space-y-3 text-sm text-slate-600">
            <!-- 检查中 -->
            <div v-if="phase === 'checking'" class="text-center py-6">
              <div
                class="animate-spin w-8 h-8 border-3 border-cyan-200 border-t-cyan-500 rounded-full mx-auto mb-3"
              ></div>
              <p>正在检查更新...</p>
            </div>

            <!-- App 更新可用 -->
            <div v-if="phase === 'app-update-available'" class="space-y-2">
              <p class="text-slate-800 font-bold">
                新版本 <span class="text-cyan-600">v{{ appVersion }}</span> 可用！
              </p>
              <div
                v-if="appReleaseNotes"
                class="max-h-32 overflow-y-auto bg-slate-50 rounded-xl p-3 text-xs whitespace-pre-wrap"
              >
                {{ appReleaseNotes }}
              </div>
            </div>

            <!-- 完成 -->
            <div v-if="phase === 'complete'" class="text-center py-4">
              <p class="text-green-600 font-bold">✓ 更新完成</p>
            </div>

            <!-- 错误 -->
            <div v-if="phase === 'error'" class="text-center py-4">
              <p class="text-red-600 font-bold">✗ 更新失败</p>
              <p class="text-xs text-slate-500 mt-1">{{ errorMessage }}</p>
            </div>
          </div>

          <!-- 按钮区域 -->
          <div class="mt-6 space-y-2">
            <!-- 有更新可用时 -->
            <button
              v-if="phase === 'app-update-available'"
              @click="$emit('update')"
              class="w-full py-4 bg-cyan-500 text-white font-black rounded-2xl shadow-lg hover:bg-cyan-600 active:scale-95 transition-all"
            >
              更新程序
            </button>

            <!-- 完成时 -->
            <button
              v-if="phase === 'complete' || phase === 'error'"
              @click="$emit('close')"
              class="w-full py-4 bg-cyan-500 text-white font-black rounded-2xl shadow-lg hover:bg-cyan-600 active:scale-95 transition-all"
            >
              {{ phase === 'complete' ? '好的' : '知道了' }}
            </button>

            <!-- 有更新可用时：稍后提醒 -->
            <button
              v-if="phase === 'app-update-available'"
              @click="$emit('later')"
              class="w-full py-3 text-slate-400 font-medium text-sm hover:text-slate-600 transition-colors"
            >
              稍后提醒
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { UpdatePhase } from '../composables/useUpdater'

const props = defineProps<{
  visible: boolean
  phase: UpdatePhase
  appVersion: string
  appReleaseNotes: string
  errorMessage: string
}>()

defineEmits<{
  update: []
  later: []
  close: []
}>()

const dialogTitle = computed(() => {
  switch (props.phase) {
    case 'checking':
      return '检查更新'
    case 'app-update-available':
      return '发现新版本'
    case 'complete':
      return '更新完成'
    case 'error':
      return '更新失败'
    default:
      return '更新'
  }
})
</script>
