<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 scale-100"
      enter-to-class="opacity-100 scale-100"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100 scale-100"
      leave-to-class="opacity-0 scale-100"
    >
      <div v-if="show" class="fixed inset-0 z-9999 flex items-center justify-center p-4">
        <!-- 背景遮罩 -->
        <div class="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" @click="close"></div>

        <!-- 模态框主体 -->
        <div class="relative z-10 bg-white w-full max-w-md rounded-[2.5rem] shadow-2xl p-8">
          <!-- 标题栏 -->
          <div class="flex justify-between items-center mb-6">
            <h3 class="text-xl font-black text-slate-800 tracking-tight">
              {{ title }}
            </h3>
            <button
              @click="close"
              class="text-slate-400 hover:text-slate-600 transition-colors p-1"
            >
              <span class="text-2xl font-bold leading-none">&times;</span>
            </button>
          </div>

          <!-- 内容区域 -->
          <div class="space-y-4">
            <slot></slot>
          </div>

          <!-- 底部按钮 -->
          <div class="mt-8">
            <button
              @click="$emit('confirm')"
              class="w-full py-4 bg-cyan-500 text-white font-black rounded-2xl shadow-lg hover:bg-cyan-600 active:scale-95 transition-all"
            >
              确认创建
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
defineProps<{
  show: boolean
  title: string
}>()

const emit = defineEmits(['close', 'confirm'])

const close = () => {
  emit('close')
}
</script>
