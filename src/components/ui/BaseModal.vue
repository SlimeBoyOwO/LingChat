<template>
  <!-- 关键点1：Teleport 必须指向 body，这样它就会直接渲染在 <body> 标签下，
       脱离任何父组件的 overflow:hidden 或 z-index 陷阱 -->
  <Teleport to="body">
    <!-- 关键点2：v-if 必须包在最外层。如果 show 为 false，整个 DOM 都不应该存在 -->
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
        <!-- z-index 必须比背景高，relative 保证它在背景之上 -->
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
              <!-- 这里如果没有引入图标，可以用 X 代替 -->
              <span class="text-2xl font-bold leading-none">&times;</span>
            </button>
          </div>

          <!-- 内容区域 -->
          <!-- 关键点3：slot 必须在 v-if 内部，否则 show=false 时插槽内容会渲染在页面底部 -->
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
// 如果这里报错找不到组件，请确保安装了 lucide-vue-next 或者删掉上面 button 里的图标
defineProps<{
  show: boolean
  title: string
}>()

const emit = defineEmits(['close', 'confirm'])

const close = () => {
  emit('close')
}
</script>
