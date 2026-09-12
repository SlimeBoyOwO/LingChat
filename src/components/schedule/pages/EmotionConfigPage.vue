<template>
  <div class="emotion-config">
    <!-- 标题 -->
    <div class="mb-6">
      <h2 class="flex items-center gap-2 text-xl font-bold tracking-tight text-white">
        <span class="h-2 w-2 animate-pulse rounded-full bg-cyan-400"></span>
        情绪模型选择
      </h2>
      <p class="mt-1 text-sm text-white/60">角色 AI 使用 ONNX 情绪分析引擎进行对话情绪识别</p>
    </div>

    <!-- 加载状态 -->
    <div v-if="loading" class="flex items-center justify-center py-12">
      <div
        class="h-8 w-8 animate-spin rounded-full border-2 border-cyan-400 border-t-transparent"
      ></div>
      <span class="ml-3 text-sm text-white/60">加载中...</span>
    </div>

    <!-- 当前模型 -->
    <div v-else class="space-y-3">
      <div
        class="relative cursor-pointer rounded-xl border border-cyan-400 bg-cyan-500/10 p-4
          shadow-[0_0_15px_rgba(121,217,255,0.15)] transition-all duration-200"
      >
        <!-- 当前使用标签 -->
        <div class="absolute top-4 right-4 flex gap-2">
          <span
            class="inline-flex items-center rounded-full border border-cyan-500/30 bg-cyan-500/20
              px-2 py-0.5 text-[11px] font-medium text-cyan-300"
          >
            当前使用
          </span>
        </div>

        <!-- 模型名称 -->
        <div class="text-base font-semibold text-white">{{ EMOTION_MODEL_LABELS.onnx.label }}</div>

        <!-- 模型说明 -->
        <div class="mt-1 text-sm leading-relaxed text-white/50">
          {{ EMOTION_MODEL_LABELS.onnx.description }}
        </div>
      </div>
    </div>

    <!-- 注意提示 -->
    <div class="mt-8 rounded-xl border border-amber-500/20 bg-amber-500/10 p-4">
      <p class="text-xs leading-relaxed text-amber-300/80">
        <span class="font-semibold text-amber-300">提示：</span>
        情绪分析由内置 ONNX 分类器全程执行，无需手动切换模型。
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref, onMounted } from "vue";
  import { getEmotionModelType, EMOTION_MODEL_LABELS } from "@/api/services/emotion";

  const loading = ref(true);

  onMounted(async () => {
    try {
      await getEmotionModelType();
    } catch {
      // 保持默认
    } finally {
      loading.value = false;
    }
  });
</script>
