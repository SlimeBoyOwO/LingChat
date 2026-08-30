<template>
  <div class="emotion-config">
    <!-- 标题 -->
    <div class="mb-6">
      <h2 class="text-xl font-bold text-white tracking-tight flex items-center gap-2">
        <span class="w-2 h-2 bg-cyan-400 rounded-full animate-pulse"></span>
        情绪模型选择
      </h2>
      <p class="text-sm text-white/60 mt-1">
        角色 AI 使用 ONNX 情绪分析引擎进行对话情绪识别
      </p>
    </div>

    <!-- 加载状态 -->
    <div v-if="loading" class="flex items-center justify-center py-12">
      <div class="w-8 h-8 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></div>
      <span class="ml-3 text-white/60 text-sm">加载中...</span>
    </div>

    <!-- 当前模型 -->
    <div v-else class="space-y-3">
      <div class="relative p-4 rounded-xl border cursor-pointer transition-all duration-200 border-cyan-400 bg-cyan-500/10 shadow-[0_0_15px_rgba(121,217,255,0.15)]">
        <!-- 当前使用标签 -->
        <div class="flex gap-2 absolute top-4 right-4">
          <span
            class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-cyan-500/20 text-cyan-300 border border-cyan-500/30"
          >
            当前使用
          </span>
        </div>

        <!-- 模型名称 -->
        <div class="font-semibold text-white text-base">{{ EMOTION_MODEL_LABELS.onnx.label }}</div>

        <!-- 模型说明 -->
        <div class="text-sm text-white/50 mt-1 leading-relaxed">
          {{ EMOTION_MODEL_LABELS.onnx.description }}
        </div>
      </div>
    </div>

    <!-- 注意提示 -->
    <div class="mt-8 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20">
      <p class="text-xs text-amber-300/80 leading-relaxed">
        <span class="font-semibold text-amber-300">提示：</span>
        情绪分析由内置 ONNX 分类器全程执行，无需手动切换模型。
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { getEmotionModelType, EMOTION_MODEL_LABELS } from '@/api/services/emotion'

const loading = ref(true)

onMounted(async () => {
  try {
    await getEmotionModelType()
  } catch {
    // 保持默认
  } finally {
    loading.value = false
  }
})
</script>