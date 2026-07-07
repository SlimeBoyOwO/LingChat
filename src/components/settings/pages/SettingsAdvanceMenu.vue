<template>
  <div class="grid grid-cols-1 md:grid-cols-2 gap-5 p-2">
    <!-- 大模型管理 -->
    <div class="cursor-pointer transition-all duration-300" @click="emit('navigate', 'llm')">
      <MenuItem title="大模型管理" size="large">
        <template #header>
          <Cpu :size="20" />
        </template>
        <p class="text-white/50 text-sm leading-relaxed mb-3">
          配置和管理 AI 大模型提供商，添加、编辑、测试模型连接参数
        </p>
        <Button type="big" icon="advance" :icon_size="18"> 进入大模型管理界面 </Button>
      </MenuItem>
    </div>

    <!-- 其他高级设置 -->
    <div class="cursor-pointer transition-all duration-300" @click="emit('navigate', 'other')">
      <MenuItem title="其他高级设置" size="large">
        <template #header>
          <SlidersHorizontal :size="20" />
        </template>
        <p class="text-white/50 text-sm leading-relaxed mb-3">
          调整系统环境配置、提示词模板、对话参数等高级选项（重启后生效）
        </p>
        <Button type="big" icon="setting" :icon_size="18"> 进入其他高级设置界面 </Button>
      </MenuItem>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Cpu, SlidersHorizontal } from 'lucide-vue-next'
import { MenuItem } from '../../ui'
import { Button } from '../../base'

const emit = defineEmits<{
  navigate: [tab: 'llm' | 'other']
}>()
</script>
