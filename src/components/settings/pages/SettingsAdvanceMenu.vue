<template>
  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5 p-2">
    <!-- 大模型管理 -->
    <div class="cursor-pointer transition-all duration-300" @click="emit('navigate', 'llm')">
      <MenuItem :title="$t('advance.menu.llmTitle')" size="large">
        <template #header>
          <Cpu :size="20" />
        </template>
        <p class="text-white/50 text-sm leading-relaxed mb-3">
          {{ $t('advance.menu.llmDesc') }}
        </p>
        <Button type="big" icon="advance" :icon_size="18">
          {{ $t('advance.menu.llmButton') }}
        </Button>
      </MenuItem>
    </div>

    <!-- 本地 TTS -->
    <div class="cursor-pointer transition-all duration-300" @click="emit('navigate', 'tts')">
      <MenuItem title="本地 TTS" size="large">
        <template #header>
          <AudioLines :size="20" />
        </template>
        <p class="text-white/50 text-sm leading-relaxed mb-3">
          导入 DeBERTa 模型、分词器和人物语音，离线使用本地 TTS 引擎
        </p>
        <Button type="big" icon="mic" :icon_size="18"> 进入本地 TTS 界面 </Button>
      </MenuItem>
    </div>

    <!-- 其他高级设置 -->
    <div class="cursor-pointer transition-all duration-300" @click="emit('navigate', 'other')">
      <MenuItem :title="$t('advance.menu.otherTitle')" size="large">
        <template #header>
          <SlidersHorizontal :size="20" />
        </template>
        <p class="text-white/50 text-sm leading-relaxed mb-3">
          {{ $t('advance.menu.otherDesc') }}
        </p>
        <Button type="big" icon="setting" :icon_size="18">
          {{ $t('advance.menu.otherButton') }}
        </Button>
      </MenuItem>
    </div>

    <!-- 界面语言 -->
    <div class="transition-all duration-300">
      <MenuItem :title="$t('advance.menu.languageTitle')" size="large">
        <template #header>
          <Languages :size="20" />
        </template>
        <p class="text-white/50 text-sm leading-relaxed mb-3">
          {{ $t('advance.menu.languageDesc') }}
        </p>
        <select
          :value="locale"
          class="w-full cursor-pointer rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm text-white/80 transition-all duration-200 hover:border-white/30 hover:text-white focus:border-[rgba(121,217,255,0.6)] focus:outline-none"
          @change="setLocale(($event.target as HTMLSelectElement).value as AppLocale)"
        >
          <option
            v-for="opt in SUPPORTED_LOCALES"
            :key="opt.value"
            :value="opt.value"
            class="bg-slate-800 text-white"
          >
            {{ opt.label }}
          </option>
        </select>
      </MenuItem>
    </div>
  </div>
</template>

<script setup lang="ts">
import { AudioLines, Cpu, SlidersHorizontal, Languages } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { MenuItem } from '../../ui'
import { Button } from '../../base'
import { SUPPORTED_LOCALES, setLocale, type AppLocale } from '@/locales'

const { locale } = useI18n()

const emit = defineEmits<{
  navigate: [tab: 'llm' | 'tts' | 'other']
}>()
</script>
