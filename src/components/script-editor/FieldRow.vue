<template>
  <div class="mb-4">
    <label class="inline-flex items-center gap-1.5 font-medium text-brand">
      {{ field.label }}
      <span
        v-if="field.required"
        class="text-xs text-red-400"
        >＊</span
      >
      <span
        v-else
        class="ml-auto text-xs font-normal text-white/35"
        >可选</span
      >
    </label>
    <p class="mt-1 mb-2 text-sm text-gray-300">{{ field.key }}</p>

    <!-- 遗留字段：只展示，不给编辑 -->
    <template v-if="field.kind === 'deprecated' || !field.enabled">
      <input
        class="glass-input opacity-45"
        :value="asText"
        disabled
      />
    </template>

    <!-- 多行文本 -->
    <textarea
      v-else-if="field.kind === 'textarea'"
      class="glass-input min-h-20 resize-y leading-relaxed"
      :value="asText"
      :placeholder="field.placeholder"
      @change="onText"
    ></textarea>

    <!-- 数字 -->
    <input
      v-else-if="field.kind === 'number'"
      class="glass-input"
      type="number"
      :value="asText"
      :placeholder="field.placeholder"
      @change="onNumber"
    />

    <!-- 开关 -->
    <div
      v-else-if="field.kind === 'bool'"
      class="flex items-center gap-2"
    >
      <Toggle
        :checked="value === true"
        @change="(v: boolean) => emit('update', v)"
      />
      <span class="text-sm text-white/70">{{ value === true ? '开启' : '关闭' }}</span>
    </div>

    <!-- 固定候选 / 角色 / 情绪 / 章节 -->
    <select
      v-else-if="isSelectLike"
      class="glass-input"
      :value="asText"
      @change="onSelect"
    >
      <option
        v-if="!field.required"
        value=""
      >
        （不设置）
      </option>
      <option
        v-for="opt in selectOptions"
        :key="opt.value"
        :value="opt.value"
      >
        {{ opt.label }}
      </option>
    </select>

    <!-- 素材：下拉 + 导入 -->
    <div v-else-if="field.kind === 'asset'">
      <div class="flex gap-2">
        <select
          class="glass-input"
          :value="asText"
          @change="onSelect"
        >
          <option
            v-if="!field.required"
            value=""
          >
            （不设置）
          </option>
          <option
            v-for="name in assetOptions"
            :key="name"
            :value="name"
          >
            {{ name }}
          </option>
        </select>
        <button
          class="shrink-0 rounded-lg border border-white/10 bg-white/6 px-3 text-sm
            whitespace-nowrap text-white/70 transition-all hover:bg-white/12 hover:text-white"
          @click="pickAsset"
        >
          导入…
        </button>
      </div>
      <p
        v-if="assetOptions.length === 0"
        class="mt-1 text-xs text-yellow-200"
      >
        这个剧本还没有该类素材，先点「导入…」
      </p>
    </div>

    <!-- 复合编辑器：选项 / 分支 / 赋值组 -->
    <CompositeField
      v-else-if="isComposite"
      :field="field"
      :value="value"
      @update="(v: unknown) => emit('update', v)"
    />

    <!-- 单行文本兜底 -->
    <input
      v-else
      class="glass-input"
      :value="asText"
      :placeholder="field.placeholder"
      @change="onText"
    />

    <p
      v-if="field.hint"
      class="mt-1 text-xs leading-relaxed"
      :class="hintClass"
    >
      {{ field.hint }}
    </p>
    <p
      v-for="(d, i) in diagnostics"
      :key="i"
      class="mt-1 text-xs leading-relaxed"
      :class="d.severity === 'error' ? 'text-red-300' : 'text-yellow-200'"
    >
      {{ d.message }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { readFile } from '@tauri-apps/plugin-fs'
import Toggle from '@/components/base/widget/Toggle.vue'
import { EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { AssetKind, Diagnostic, FieldSpec } from '@/api/services/script-editor'
import CompositeField from './CompositeField.vue'

const props = defineProps<{
  field: FieldSpec
  value: unknown
  diagnostics: Diagnostic[]
}>()

const emit = defineEmits<{ (e: 'update', value: unknown): void }>()

const store = useScriptEditorStore()

const asText = computed(() => {
  const v = props.value
  if (v === undefined || v === null) return ''
  if (typeof v === 'object') return ''
  return String(v)
})

const isSelectLike = computed(() =>
  ['select', 'character', 'emotion', 'chapter'].includes(props.field.kind),
)

const isComposite = computed(() =>
  ['choice_options', 'branch_options', 'var_options'].includes(props.field.kind),
)

/**
 * 候选项的归属：
 * - select   → Rust schema 给的固定表（如背景特效）
 * - character→ MAIN + 剧本内 NPC
 * - emotion  → **前端**的情绪表（它决定情绪到立绘文件名的映射，归前端所有）
 * - chapter  → 当前剧本的章节列表 + 「剧本结束」
 */
const selectOptions = computed<{ value: string; label: string }[]>(() => {
  switch (props.field.kind) {
    case 'select':
      return (props.field.options ?? []).map((o) => ({ value: o, label: o }))
    case 'character':
      return store.characterOptions.map((o) => ({
        value: o,
        label: o === 'MAIN' ? 'MAIN（当前主角）' : o,
      }))
    case 'emotion':
      return Object.keys(EMOTION_CONFIG_EMO).map((o) => ({ value: o, label: o }))
    case 'chapter':
      return store.chapterOptions
    default:
      return []
  }
})

const assetOptions = computed<string[]>(() => {
  const kind = props.field.assetKind
  if (!kind) return []
  return store.assets[kind] ?? []
})

const hintClass = computed(() =>
  /⚠|不生效|不会|无效|卡死/.test(props.field.hint ?? '') ? 'text-yellow-200' : 'text-white/40',
)

const onText = (e: Event) => emit('update', (e.target as HTMLInputElement).value)

const onSelect = (e: Event) => emit('update', (e.target as HTMLSelectElement).value)

const onNumber = (e: Event) => {
  const raw = (e.target as HTMLInputElement).value.trim()
  if (raw === '') {
    emit('update', '')
    return
  }
  const n = Number(raw)
  // 不是数字就别往 YAML 里写 —— 引擎读到字符串会静默回落默认值
  emit('update', Number.isFinite(n) ? n : '')
}

const IMAGE_EXT = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif']
const AUDIO_EXT = ['mp3', 'wav', 'ogg', 'flac', 'm4a']

const pickAsset = async () => {
  const kind = props.field.assetKind as AssetKind | undefined
  if (!kind) return
  const isImage = kind === 'background' || kind === 'pic'
  const picked = await openDialog({
    multiple: false,
    filters: [
      {
        name: isImage ? '图片' : '音频',
        extensions: isImage ? IMAGE_EXT : AUDIO_EXT,
      },
    ],
  })
  if (typeof picked !== 'string') return

  const bytes = await readFile(picked)
  const fileName = picked.split(/[/\\]/).pop() ?? 'asset'
  await store.uploadAsset(kind, fileName, bytes)
  // 导入成功后直接选中它，省掉一次手动选择
  if (store.assets[kind].includes(fileName)) emit('update', fileName)
}
</script>

<style scoped>
.glass-input {
  width: 100%;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 0.5rem;
  background: rgba(255, 255, 255, 0.1);
  padding: 0.625rem 0.75rem;
  font-size: 0.875rem;
  color: #fff;
  backdrop-filter: blur(20px) saturate(150%);
  transition: all 0.2s;
}
.glass-input:focus {
  outline: none;
  border-color: var(--accent-color);
  box-shadow: 0 0 0 2px rgba(121, 217, 255, 0.2);
}
.glass-input option {
  background: #16202c;
  color: #fff;
}
</style>
