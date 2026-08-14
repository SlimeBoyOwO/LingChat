<template>
  <div class="mb-4">
    <label
      class="inline-flex
        items-center
        gap-1.5
        font-medium
        text-brand"
      :title="t('scriptEditor.fieldRow.yamlKey', { key: field.key })"
    >
      {{ fieldLabelOf(field, eventType) }}
      <span
        v-if="field.required"
        class="text-xs
          text-red-400"
        >{{ t('scriptEditor.configTab.requiredMark') }}</span
      >
      <span
        v-else
        class="ml-auto
          text-xs
          font-normal
          text-white/35"
        >{{ t('scriptEditor.fieldRow.optional') }}</span
      >
    </label>

    <!-- 遗留字段：只展示，不给编辑 -->
    <template v-if="field.kind === 'deprecated' || !field.enabled">
      <input
        class="glass-input
          opacity-45"
        :value="asText"
        disabled
      />
    </template>

    <!-- 多行文本 -->
    <textarea
      v-else-if="field.kind === 'textarea'"
      class="glass-input
        min-h-20
        resize-y
        leading-relaxed"
      :value="asText"
      :placeholder="fieldPlaceholderOf(field, eventType)"
      @change="onText"
    ></textarea>

    <!-- 数字 -->
    <input
      v-else-if="field.kind === 'number'"
      class="glass-input"
      type="number"
      :value="asText"
      :placeholder="fieldPlaceholderOf(field, eventType)"
      @change="onNumber"
    />

    <!-- 开关。必填字段只有开/关两态；可选字段必须能表达「不设置」——
         引擎对这类字段的默认值往往不是 false（比如环境音的 loop / fade 默认 true），
         用两态开关会让「没写过这个字段」和「显式写了 false」长得一模一样，
         作者点一下再点回来就悄悄改变了行为。 -->
    <div
      v-else-if="field.kind === 'bool' && field.required"
      class="flex
        items-center
        gap-2"
    >
      <Toggle
        :checked="value === true"
        @change="(v: boolean) => emit('update', v)"
      />
      <span class="text-sm
        text-white/70">{{
        value === true ? t('scriptEditor.fieldRow.on') : t('scriptEditor.fieldRow.off')
      }}</span>
    </div>
    <select
      v-else-if="field.kind === 'bool'"
      class="glass-input"
      :value="value === true ? 'true' : value === false ? 'false' : ''"
      @change="onTriState"
    >
      <option value="">
        {{
          field.defaultDesc
            ? t('scriptEditor.fieldRow.notSetDefaultWith', { default: field.defaultDesc })
            : t('scriptEditor.fieldRow.notSetDefault')
        }}
      </option>
      <option value="true">{{ t('scriptEditor.fieldRow.on') }}</option>
      <option value="false">{{ t('scriptEditor.fieldRow.off') }}</option>
    </select>

    <!-- 固定候选 / 角色 / 情绪 / 章节 -->
    <select
      v-else-if="isSelectLike"
      class="glass-input"
      :value="asText"
      @change="onSelect"
    >
      <!-- 非必填字段：不设置。character 例外：独立剧本无 MAIN 选项时，
           留空即引擎的 MAIN（当前主角），故也提供空选项并明确提示 -->
      <option
        v-if="!field.required || field.kind === 'character'"
        value=""
      >
        {{
          field.kind === 'character'
            ? t('scriptEditor.fieldRow.characterEmpty')
            : t('scriptEditor.configTab.notSet')
        }}
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
      <div class="flex
        gap-2">
        <select
          class="glass-input"
          :value="asText"
          @change="onSelect"
        >
          <option
            v-if="!field.required"
            value=""
          >
            {{ t('scriptEditor.configTab.notSet') }}
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
          class="shrink-0
            border
            border-white/[0.1]
            rounded-lg
            px-[0.7rem]
            text-[0.78rem]
            whitespace-nowrap
            text-white/[0.7]
            bg-white/[0.06]
            transition-all
            hover:text-white
            hover:bg-white/[0.14]"
          :title="t('scriptEditor.fieldRow.importScript')"
          @click="pickAsset('script')"
        >
          {{ t('scriptEditor.assets.importScript') }}
        </button>
        <button
          class="shrink-0
            rounded-lg
            px-[0.7rem]
            text-[0.78rem]
            whitespace-nowrap
            transition-all
            border
            border-[rgba(167,139,250,0.3)]
            text-[#c4b5fd]
            bg-[rgba(167,139,250,0.1)]
            hover:bg-[rgba(167,139,250,0.22)]"
          :title="t('scriptEditor.fieldRow.importGlobal')"
          @click="pickAsset('global')"
        >
          {{ t('scriptEditor.assets.importGlobal') }}
        </button>
      </div>
      <p
        v-if="assetOptions.length === 0"
        class="mt-1
          text-xs
          text-yellow-200"
      >
        {{ t('scriptEditor.fieldRow.noAssetsHint') }}
      </p>
      <p
        v-else-if="globalOnly.length"
        class="mt-1
          text-xs
          text-white/35"
      >
        {{ t('scriptEditor.fieldRow.globalOnlyHint', { count: globalOnly.length }) }}
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

    <!-- 服装：按事件所选角色动态生成候选（剧本 NPC 服装 + MAIN 绑定角色的全局服装） -->
    <select
      v-else-if="field.key === 'clothes'"
      class="glass-input"
      :value="asText"
      @change="onSelect"
    >
      <option
        v-if="!field.required"
        value=""
      >
        {{ t('scriptEditor.configTab.notSet') }}
      </option>
      <option
        v-for="opt in clothesOptions"
        :key="opt.value"
        :value="opt.value"
      >
        {{ opt.label }}
      </option>
    </select>

    <!-- 单行文本兜底 -->
    <input
      v-else
      class="glass-input"
      :value="asText"
      :placeholder="fieldPlaceholderOf(field, eventType)"
      @change="onText"
    />

    <p
      v-if="field.hint"
      class="mt-1
        text-xs
        leading-relaxed"
      :class="hintClass"
    >
      {{ fieldHintOf(field, eventType) }}
    </p>
    <p
      v-for="(d, i) in diagnostics"
      :key="i"
      class="mt-1
        text-xs
        leading-relaxed"
      :class="d.severity === 'error' ? 'text-red-300' : 'text-yellow-200'"
    >
      {{ d.message }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { Toggle } from '@/components/base'
import { EMOTION_CONFIG_EMO } from '@/controllers/emotion/config'
import {
  emotionLabelOf,
  fieldHintOf,
  fieldLabelOf,
  fieldPlaceholderOf,
  optionLabelOf,
  particleLabelOf,
} from '@/locales/schema-i18n'
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

const { t } = useI18n()
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

/**
 * 服装下拉候选：
 * - 先固定一个「默认（不进子目录）」—— 引擎把 default / 空都映射为不进子目录；
 * - 剧本 NPC：当前事件 character 字段对应角色的 avatar/ 子目录；
 * - MAIN（或未选角色）：绑定羁绊人物的全局角色服装（后端 editor_list_global_characters 已返回）。
 */
const clothesOptions = computed<{ value: string; label: string }[]>(() => {
  const base: { value: string; label: string }[] = [
    { value: 'default', label: t('scriptEditor.fieldRow.clothesDefault') },
  ]
  const seen = new Set<string>(['default'])
  const add = (name: string) => {
    if (!name || seen.has(name)) return
    seen.add(name)
    base.push({ value: name, label: name })
  }

  const charKey = typeof props.event?.character === 'string' ? props.event.character : ''
  const npc = store.detail?.characters.find((c) => c.roleKey === charKey || c.folder === charKey)
  npc?.clothes.forEach(add)

  if (!charKey || charKey === 'MAIN') {
    const bound = store.detail?.package.boundCharacterFolder
    const gc = store.globalCharacters.find((g) => g.folder === bound)
    gc?.clothes.forEach(add)
  }
  return base
})

/** 当前事件类型（schema 词条映射用）；无 event 时为 undefined，按 fieldKey 查通用表 */
const eventType = computed(() =>
  typeof props.event?.type === 'string' ? props.event.type : undefined,
)

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
      return (props.field.options ?? []).map((o, idx) => {
        const value = typeof o === 'string' ? o : o.value
        const raw = typeof o === 'string' ? (props.field.optionLabels?.[idx] ?? o) : o.label
        // 背景特效：options 已被前端覆盖为粒子表，label 是前端中文 → 走粒子词条
        if (props.field.key === 'effect') return { value, label: particleLabelOf(value, raw) }
        return { value, label: optionLabelOf(props.field, eventType.value, value, idx) }
      })
    case 'character':
      // store.characterOptions 已含 label（MAIN → 绑定角色名，NPC → aiName）
      return store.characterOptions
    case 'emotion':
      return Object.keys(EMOTION_CONFIG_EMO).map((o) => ({ value: o, label: emotionLabelOf(o) }))
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
  /⚠/.test(props.field.hint ?? '') ? 'text-yellow-200' : 'text-white/40',
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
        name: isImage ? t('scriptEditor.fieldRow.image') : t('scriptEditor.fieldRow.audio'),
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
