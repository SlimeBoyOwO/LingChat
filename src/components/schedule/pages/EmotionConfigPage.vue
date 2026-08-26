<template>
  <div class="emotion-config">
    <!-- 标题 -->
    <div class="mb-6">
      <h2 class="text-xl font-bold text-white tracking-tight flex items-center gap-2">
        <span class="w-2 h-2 bg-cyan-400 rounded-full animate-pulse"></span>
        情绪模型选择
      </h2>
      <p class="text-sm text-white/60 mt-1">
        选择角色 AI 使用的情绪分析引擎，保存后重启对话生效
      </p>
    </div>

    <!-- 加载状态 -->
    <div v-if="loading" class="flex items-center justify-center py-12">
      <div class="w-8 h-8 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></div>
      <span class="ml-3 text-white/60 text-sm">加载中...</span>
    </div>

    <!-- 模型选择卡片 -->
    <div v-else class="space-y-3">
      <div
        v-for="item in modelOptions"
        :key="item.type"
        @click="selected = item.type"
        class="group relative p-4 rounded-xl border cursor-pointer transition-all duration-200"
        :class="[
          selected === item.type
            ? 'border-cyan-400 bg-cyan-500/10 shadow-[0_0_15px_rgba(121,217,255,0.15)]'
            : 'border-white/10 bg-white/5 hover:border-white/20 hover:bg-white/10',
        ]"
      >
        <!-- 选中指示器 -->
        <div
          class="absolute top-4 right-4 w-5 h-5 rounded-full border-2 flex items-center justify-center transition-all duration-200"
          :class="[
            selected === item.type
              ? 'border-cyan-400 bg-cyan-400'
              : 'border-white/30',
          ]"
        >
          <div
            v-if="selected === item.type"
            class="w-2 h-2 rounded-full bg-white"
          ></div>
        </div>

        <!-- 模型名称 -->
        <div class="font-semibold text-white text-base">{{ item.label }}</div>

        <!-- 模型说明 -->
        <div class="text-sm text-white/50 mt-1 leading-relaxed">
          {{ item.description }}
        </div>

        <!-- 状态标签 -->
        <div class="flex gap-2 mt-2">
          <span
            v-if="item.type === '9d'"
            class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-purple-500/20 text-purple-300 border border-purple-500/30"
          >
            新！
          </span>
          <span
            v-if="item.type === item.currentType"
            class="inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-medium bg-cyan-500/20 text-cyan-300 border border-cyan-500/30"
          >
            当前使用
          </span>
        </div>
      </div>
    </div>

    <!-- 保存按钮 -->
    <div class="mt-6 flex items-center gap-3">
      <button
        @click="handleSave"
        :disabled="saving || selected === currentType"
        class="px-6 py-2.5 rounded-xl font-medium text-sm transition-all duration-200 disabled:opacity-40 disabled:cursor-not-allowed"
        :class="
          saving
            ? 'bg-cyan-600 text-white/70'
            : 'bg-cyan-500 hover:bg-cyan-600 text-white shadow-lg hover:shadow-cyan-500/25 active:scale-[0.97]'
        "
      >
        <span v-if="saving" class="flex items-center gap-2">
          <div class="w-4 h-4 border-2 border-white/60 border-t-transparent rounded-full animate-spin"></div>
          保存中...
        </span>
        <span v-else>保存设置</span>
      </button>

      <!-- 保存成功提示 -->
      <transition
        enter-active-class="transition-all duration-300 ease-out"
        leave-active-class="transition-all duration-200 ease-in"
        enter-from-class="opacity-0 translate-y-1"
        leave-to-class="opacity-0"
      >
        <span
          v-if="showSaved"
          class="text-green-400 text-sm font-medium"
        >
          ✅ 已保存
        </span>
      </transition>
    </div>

    <!-- 注意提示 -->
    <div class="mt-8 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20">
      <p class="text-xs text-amber-300/80 leading-relaxed">
        <span class="font-semibold text-amber-300">提示：</span>
        切换模型类型即刻生效，无需重启。关闭情绪分类后，角色将使用默认情绪状态进行交互。
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import {
  getEmotionModelType,
  setEmotionModelLive,
  EMOTION_MODEL_LABELS,
  type EmotionModelType,
} from '@/api/services/emotion'

const loading = ref(true)
const saving = ref(false)
const showSaved = ref(false)

const currentType = ref<EmotionModelType>('9d')
const selected = ref<EmotionModelType>('9d')

interface ModelOption {
  type: EmotionModelType
  label: string
  description: string
  currentType: EmotionModelType
}

const modelOptions = computed<ModelOption[]>(() =>
  (['9d', 'onnx', 'disabled'] as EmotionModelType[]).map((type) => ({
    type,
    ...EMOTION_MODEL_LABELS[type],
    currentType: currentType.value,
  })),
)

onMounted(async () => {
  try {
    const type = await getEmotionModelType()
    currentType.value = type
    selected.value = type
  } catch {
    // Keep defaults
  } finally {
    loading.value = false
  }
})

async function handleSave() {
  if (selected.value === currentType.value) return
  saving.value = true
  showSaved.value = false
  try {
    await setEmotionModelLive(selected.value)
    currentType.value = selected.value
    showSaved.value = true
    setTimeout(() => {
      showSaved.value = false
    }, 3000)
  } catch (e) {
    console.error('保存情绪模型设置失败:', e)
  } finally {
    saving.value = false
  }
}
</script>
