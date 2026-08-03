<template>
  <div class="mb-4">
    <label
      class="inline-flex items-center gap-1.5 font-medium text-brand"
      :title="`YAML 字段名：${field.key}`"
    >
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

    <!-- 开关。必填字段只有开/关两态；可选字段必须能表达「不设置」——
         引擎对这类字段的默认值往往不是 false（比如环境音的 loop / fade 默认 true），
         用两态开关会让「没写过这个字段」和「显式写了 false」长得一模一样，
         作者点一下再点回来就悄悄改变了行为。 -->
    <div
      v-else-if="field.kind === 'bool' && field.required"
      class="flex items-center gap-2"
    >
      <Toggle
        :checked="value === true"
        @change="(v: boolean) => emit('update', v)"
      />
      <span class="text-sm text-white/70">{{ value === true ? '开启' : '关闭' }}</span>
    </div>
    <select
      v-else-if="field.kind === 'bool'"
      class="glass-input"
      :value="value === true ? 'true' : value === false ? 'false' : ''"
      @change="onTriState"
    >
      <option value="">（不设置 · 用引擎默认值）</option>
      <option value="true">开启</option>
      <option value="false">关闭</option>
    </select>

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
          class="shrink-0 border border-white/[0.1] rounded-lg px-[0.7rem] text-[0.78rem] whitespace-nowrap text-white/[0.7] bg-white/[0.06] transition-all hover:text-white hover:bg-white/[0.14]"
          title="导入到本剧本 —— 随剧本一起分发，别的剧本看不到"
          @click="pickAsset('script')"
        >
          导入
        </button>
        <button
          class="shrink-0 rounded-lg px-[0.7rem] text-[0.78rem] whitespace-nowrap transition-all border border-[rgba(167,139,250,0.3)] text-[#c4b5fd] bg-[rgba(167,139,250,0.1)] hover:bg-[rgba(167,139,250,0.22)]"
          title="导入为全局素材 —— 所有剧本共享，但导出剧本时不会带走"
          @click="pickAsset('global')"
        >
          全局
        </button>
      </div>
      <p
        v-if="assetOptions.length === 0"
        class="mt-1 text-xs text-yellow-200"
      >
        没有可用素材。「导入」放进本剧本，「全局」放进 game_data 供所有剧本共享。
      </p>
      <p
        v-else-if="globalOnly.length"
        class="mt-1 text-xs text-white/35"
      >
        其中 {{ globalOnly.length }} 个来自全局素材库
      </p>
    </div>

    <!-- 触发条件：结构化「变量 + 关系 + 值」表单，无需手写语法。
         说明由下方 schema 的 hint 承担，隐藏编辑器自带的顶部行，避免重复 -->
    <ConditionEditor
      v-else-if="field.kind === 'condition'"
      :model-value="asText"
      :variables="store.variables"
      :hint="''"
      @update:model-value="(v: string) => emit('update', v)"
    />

    <!-- 复合编辑器：选项 / 分支 / 赋值组 -->
    <CompositeField
      v-else-if="isComposite"
      :field="field"
      :value="value"
      :branch-mode="branchMode"
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
import { Toggle } from '@/components/base'
import { EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type {
  AssetKind,
  AssetScope,
  Diagnostic,
  FieldSpec,
  ScriptEventData,
} from '@/api/services/script-editor'
import CompositeField from './CompositeField.vue'
import ConditionEditor from './ConditionEditor.vue'

const props = defineProps<{
  field: FieldSpec
  value: unknown
  /** 整个事件对象。分支编辑器需要看兄弟字段 end_type 才知道要不要显示 AI 分支名 */
  event?: ScriptEventData
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
      // 有 option_labels 用显示名（值仍是引擎认的原文），否则直接显示原文。
      // Rust 侧序列化出来的一定是字符串，这里收窄类型免得 TS 抱怨。
      return (props.field.options ?? []).map((o, idx) => ({
        value: typeof o === 'string' ? o : o.value,
        label: typeof o === 'string' ? (props.field.optionLabels?.[idx] ?? o) : o.label,
      }))
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

/**
 * 素材候选 = 本剧本 + 全局，去重合并。
 *
 * 引擎的查找顺序是「先本剧本 Assets/，再全局 game_data/」，两处的文件都能被
 * 找到，所以下拉里必须都列出来 —— 否则作者会以为全局素材在剧本里用不了。
 */
const scriptAssets = computed<string[]>(() => {
  const kind = props.field.assetKind
  return kind ? (store.assets[kind] ?? []) : []
})

const globalOnly = computed<string[]>(() => {
  const kind = props.field.assetKind
  if (!kind) return []
  const own = new Set(scriptAssets.value)
  return (store.globalAssets[kind] ?? []).filter((n) => !own.has(n))
})

const assetOptions = computed<string[]>(() => [...scriptAssets.value, ...globalOnly.value])

/**
 * 分支列表显示模式：按 end_type 决定分支编辑器给作者看「条件」还是「AI 识别名」。
 * - branching → 条件（引擎按 condition 选分支）
 * - ai_judged → AI 识别名（引擎按 name 选分支），条件不读
 * linear 或未设置时无分支，给 undefined 由 CompositeField 兜底为 branching。
 */
const branchMode = computed<'branching' | 'ai_judged' | undefined>(() => {
  const et = props.event?.end_type
  return et === 'ai_judged' ? 'ai_judged' : et === 'branching' ? 'branching' : undefined
})

const hintClass = computed(() =>
  /⚠|不生效|不会|无效|卡死/.test(props.field.hint ?? '') ? 'text-yellow-200' : 'text-white/40',
)

const onText = (e: Event) => emit('update', (e.target as HTMLInputElement).value)

const onSelect = (e: Event) => emit('update', (e.target as HTMLSelectElement).value)

/** 空串会被 store 的 setEventField 当成「删键」，正好就是「不设置」的语义 */
const onTriState = (e: Event) => {
  const v = (e.target as HTMLSelectElement).value
  emit('update', v === '' ? '' : v === 'true')
}

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

/**
 * 选一个文件导入。只把**路径**交给后端，由 Rust 自己复制 —— 与
 * `import_font` / `importRoleFromPath` 的既有做法一致。
 *
 * 不用 `plugin-fs` 读字节：用户从任意位置选的文件不在 capabilities 的
 * `fs:scope` 内会被插件直接拒绝，而且大文件转成数字数组走 IPC 会 OOM。
 */
const pickAsset = async (scope: AssetScope) => {
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

  // 用后端返回的名字而不是源文件名 —— Rust 会做一次名称清洗，两者可能不同
  const saved = await store.uploadAsset(kind, scope, picked)
  if (saved) emit('update', saved)
}
</script>
