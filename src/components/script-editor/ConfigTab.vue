<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Button, Icon, Toggle } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import {
  storyFieldHintOf,
  storyFieldLabelOf,
  unlockFieldLabelOf,
  unlockFieldPlaceholderOf,
  unlockTypeLabelOf,
} from '@/locales/schema-i18n'
import type { UnlockConditionSpec } from '@/api/services/script-editor'

const { t } = useI18n()
const store = useScriptEditorStore()

const configDraft = reactive<Record<string, unknown>>({})

watch(
  () => store.detail?.storyConfig,
  (cfg) => {
    for (const k of Object.keys(configDraft)) delete configDraft[k]
    Object.assign(configDraft, JSON.parse(JSON.stringify(cfg ?? {})))
  },
  { immediate: true, deep: false },
)

const setConfig = (key: string, value: unknown) => {
  configDraft[key] = value
}

const adventureObj = computed<Record<string, unknown>>(() => {
  const a = configDraft.adventure
  return a && typeof a === 'object' ? (a as Record<string, unknown>) : {}
})

const isAdventure = computed(() => adventureObj.value.is_adventure === true)

const adventureField = (k: string) => {
  const v = adventureObj.value[k]
  return v === undefined || v === null ? '' : String(v)
}

const setAdventure = (k: string, v: unknown) => {
  const next = { ...adventureObj.value, [k]: v }
  configDraft.adventure = next
}

/** 抽出来是因为内联写法要带 `(e.target as HTMLInputElement)`，模板里读起来太吵 */
const onAdventureText = (k: string, e: Event) =>
  setAdventure(k, (e.target as HTMLInputElement).value)

const onAdventureNumber = (k: string, e: Event) =>
  setAdventure(k, Number((e.target as HTMLInputElement).value) || 0)

/** 全局角色目录名集合，供「绑定角色」下拉判断当前值是否还在角色库里 */
const knownBoundFolders = computed(() => new Set(store.globalCharacters.map((g) => g.folder)))

/** 当前已填的绑定角色；老剧本可能手填过角色库外的目录名，下拉里补一个选项原样保留 */
const currentBound = computed(() => adventureField('bound_character_folder'))

const toggleAdventure = (on: boolean) => {
  if (on) {
    setAdventure('is_adventure', true)
  } else {
    // 关掉只改标志，其余字段原样留着 —— 作者可能只是临时关掉
    setAdventure('is_adventure', false)
  }
}

// ========================================================
// 解锁条件可视化编辑
// ========================================================

const unlockSpecs = computed<UnlockConditionSpec[]>(() => store.schema?.unlockConditionTypes ?? [])

/** 当前编辑中的解锁条件（YAML 未配置时为空数组） */
const conditions = computed<Record<string, unknown>[]>(() => {
  const c = adventureObj.value.unlock_conditions
  return Array.isArray(c) ? (c as Record<string, unknown>[]) : []
})

/** 类型 spec 查找表：type_key → spec */
const unlockSpecByType = computed(() => {
  const m = new Map<string, UnlockConditionSpec>()
  for (const s of unlockSpecs.value) m.set(s.typeKey, s)
  return m
})

const condField = (cond: Record<string, unknown>, key: string) => {
  const v = cond[key]
  return v === undefined || v === null ? '' : String(v)
}

/** 可供前置的羁绊冒险（排除当前剧本自己）：显示名（目录名），值 = 目录名（引擎按它匹配完成记录） */
const adventureOptions = computed(() =>
  store.scripts
    .filter((p) => p.isAdventure && p.folderName !== store.detail?.package.folderName)
    .map((p) => ({
      value: p.folderName,
      label: p.scriptName
        ? t('scriptEditor.configTab.prefixLabel', { name: p.scriptName, folder: p.folderName })
        : p.folderName,
    })),
)

const adventureOptionValues = computed(() => adventureOptions.value.map((o) => o.value))

const onCondType = (i: number, e: Event) => {
  const t = (e.target as HTMLSelectElement).value
  const next = [...conditions.value]
  // 换类型时清掉旧类型的字段，避免残留
  next[i] = { type: t }
  setAdventure('unlock_conditions', next)
}

const onCondField = (i: number, key: string, v: unknown) => {
  const next = [...conditions.value]
  const cond = { ...(next[i] ?? {}) }
  if (v === '') delete cond[key]
  else cond[key] = v
  next[i] = cond
  setAdventure('unlock_conditions', next)
}

const onCondNumber = (i: number, key: string, e: Event) => {
  const n = Number((e.target as HTMLInputElement).value)
  onCondField(i, key, Number.isFinite(n) ? n : '')
}

const addCondition = () => {
  const first = unlockSpecs.value[0]
  const next = [...conditions.value, { type: first?.typeKey ?? 'chat_count' }]
  setAdventure('unlock_conditions', next)
}

const removeCondition = (i: number) => {
  const next = [...conditions.value]
  next.splice(i, 1)
  if (next.length === 0) {
    // 删光后连键一起去掉，等价于「无解锁条件 = 默认解锁」
    const adv = { ...adventureObj.value }
    delete adv.unlock_conditions
    configDraft.adventure = adv
  } else {
    setAdventure('unlock_conditions', next)
  }
}

const saveConfig = () => {
  void store.saveStoryConfig(JSON.parse(JSON.stringify(configDraft)))
}
</script>

<template>
  <MenuPage>
    <MenuItem :title="t('scriptEditor.config.menuTitle')">
      <template #header>
        <Icon
          icon="setting"
          :size="20"
        />
      </template>

      <div
        v-for="f in store.schema?.storyConfigFields ?? []"
        :key="f.key"
        class="mb-4"
      >
        <label class="inline-flex
          items-center
          font-medium
          text-brand
          text-[0.9rem]">
          {{ storyFieldLabelOf(f)
          }}<span
            v-if="f.required"
            class="ml-0.5
              text-[0.7rem]
              text-red-400"
            >{{ t('scriptEditor.configTab.requiredMark') }}</span
          >
        </label>
        <p class="my-1
          mb-2
          text-[0.8rem]
          text-gray-300">{{ f.key }}</p>
        <select
          v-if="f.kind === 'chapter'"
          class="glass-input"
          :value="configDraft[f.key] ?? ''"
          @change="(e) => setConfig(f.key, (e.target as HTMLSelectElement).value)"
        >
          <option
            v-for="c in store.chapterOptions.filter((o) => o.value !== 'end')"
            :key="c.value"
            :value="c.value"
          >
            {{ c.label }}
          </option>
        </select>
        <textarea
          v-else-if="f.kind === 'textarea'"
          class="glass-input
            min-h-16"
          :value="String(configDraft[f.key] ?? '')"
          @change="(e) => setConfig(f.key, (e.target as HTMLTextAreaElement).value)"
        ></textarea>
        <input
          v-else
          class="glass-input"
          :value="String(configDraft[f.key] ?? '')"
          @change="(e) => setConfig(f.key, (e.target as HTMLInputElement).value)"
        />
        <p
          v-if="f.hint"
          class="mt-[0.3rem]
            text-[0.72rem]
            leading-[1.7]
            text-white/40
            [&_code]:font-mono
            [&_code]:text-brand"
        >
          {{ storyFieldHintOf(f) }}
        </p>
      </div>

      <!-- 羁绊冒险 -->
      <div class="my-4
        rounded-xl
        border
        border-white/10
        bg-black/15
        p-4">
        <label
          class="inline-flex
            items-center
            gap-2
            text-[0.8rem]
            whitespace-nowrap
            text-white/70
            mb-2"
        >
          <Toggle
            :checked="isAdventure"
            @change="toggleAdventure"
          />
          {{ t('scriptEditor.configTab.isAdventure') }}
        </label>
        <template v-if="isAdventure">
          <div class="mb-4">
            <label class="inline-flex
              items-center
              font-medium
              text-brand
              text-[0.9rem]">{{
              t('scriptEditor.configTab.boundCharacter')
            }}</label>
            <p class="my-1
              mb-2
              text-[0.8rem]
              text-gray-300">adventure.bound_character_folder</p>
            <!-- 下拉直选全局角色库的人物；角色多时浏览器自带滚动，不会溢出 -->
            <select
              class="glass-input"
              :value="currentBound"
              @change="(e) => onAdventureText('bound_character_folder', e)"
            >
              <option value="">{{ t('scriptEditor.configTab.notSet') }}</option>
              <option
                v-if="currentBound && !knownBoundFolders.has(currentBound)"
                :value="currentBound"
              >
                {{ currentBound }}
              </option>
              <option
                v-for="g in store.globalCharacters"
                :key="g.folder"
                :value="g.folder"
              >
                {{ t('scriptEditor.configTab.prefixLabel', { name: g.aiName, folder: g.folder }) }}
              </option>
            </select>
            <p
              v-if="store.globalCharacters.length === 0"
              class="mt-[0.3rem]
                text-[0.72rem]
                leading-[1.7]
                text-yellow-200"
            >
              {{ t('scriptEditor.configTab.emptyGlobalCharacters') }}
            </p>
          </div>
          <div class="mb-4">
            <label class="inline-flex
              items-center
              font-medium
              text-brand
              text-[0.9rem]">{{
              t('scriptEditor.configTab.order')
            }}</label>
            <p class="my-1
              mb-2
              text-[0.8rem]
              text-gray-300">adventure.order</p>
            <input
              class="glass-input"
              type="number"
              :value="adventureField('order')"
              @change="(e) => onAdventureNumber('order', e)"
            />
            <p class="mt-[0.3rem]
              text-[0.72rem]
              leading-[1.7]
              text-white/40">
              {{ t('scriptEditor.configTab.orderHint') }}
            </p>
          </div>

          <!-- 解锁条件：可视化编辑 -->
          <div class="mb-4">
            <label class="inline-flex
              items-center
              font-medium
              text-brand
              text-[0.9rem]">{{
              t('scriptEditor.configTab.unlockConditions')
            }}</label>
            <p class="my-1
              mb-2
              text-[0.8rem]
              text-gray-300">adventure.unlock_conditions</p>
            <p
              class="mb-2
                text-[0.72rem]
                leading-[1.7]
                text-white/40
                [&_b]:font-semibold
                [&_b]:text-white/85"
              v-html="t('scriptEditor.configTab.unlockHint')"
            ></p>
            <div
              v-for="(cond, i) in conditions"
              :key="i"
              class="mb-2
                rounded-lg
                bg-white/6
                p-2.5"
            >
              <div class="flex
                items-center
                gap-2">
                <select
                  class="glass-input"
                  :value="String(cond.type ?? '')"
                  @change="(e) => onCondType(i, e)"
                >
                  <option
                    v-for="s in unlockSpecs"
                    :key="s.typeKey"
                    :value="s.typeKey"
                  >
                    {{ unlockTypeLabelOf(s) }}
                  </option>
                </select>
                <button
                  class="shrink-0
                    rounded-md
                    px-1.5
                    py-1
                    text-xs
                    text-white/[0.35]
                    transition-all
                    hover:text-[#fca5a5]
                    hover:bg-[rgba(248,113,113,0.15)]"
                  :title="t('scriptEditor.config.deleteCondition')"
                  @click="removeCondition(i)"
                >
                  ✕
                </button>
              </div>
              <div
                v-for="f in unlockSpecByType.get(String(cond.type ?? ''))?.fields ?? []"
                :key="f.key"
                class="mt-2
                  flex
                  items-center
                  gap-2
                  pl-6"
              >
                <span class="shrink-0
                  text-xs
                  text-white/40">{{ unlockFieldLabelOf(f) }}</span>
                <input
                  v-if="f.kind === 'number'"
                  class="w-24
                    min-w-0
                    border
                    border-white/[0.1]
                    rounded-md
                    bg-black/[0.25]
                    px-2
                    py-1.5
                    text-xs
                    text-white
                    transition-all
                    focus:outline-none
                    focus:border-[var(--accent-color)]"
                  type="number"
                  :value="condField(cond, f.key)"
                  @change="(e) => onCondNumber(i, f.key, e)"
                />
                <!-- 成就条件：下拉直选已有成就，不用记英文键名 -->
                <select
                  v-else-if="
                    f.key === 'achievement_id' && String(cond.type ?? '') === 'achievement_unlocked'
                  "
                  class="flex-1
                    min-w-0
                    border
                    border-white/[0.1]
                    rounded-md
                    bg-black/[0.25]
                    px-2
                    py-1.5
                    text-xs
                    text-white
                    transition-all
                    focus:outline-none
                    focus:border-[var(--accent-color)]"
                  :value="condField(cond, f.key)"
                  @change="(e) => onCondField(i, f.key, (e.target as HTMLSelectElement).value)"
                >
                  <option value="">{{ t('scriptEditor.configTab.notSet') }}</option>
                  <option
                    v-if="condField(cond, f.key) && !(condField(cond, f.key) in store.achievements)"
                    :value="condField(cond, f.key)"
                  >
                    {{ condField(cond, f.key) }}
                  </option>
                  <option
                    v-for="(title, id) in store.achievements"
                    :key="id"
                    :value="id"
                  >
                    {{ title }}（{{ id }}）
                  </option>
                </select>
                <!-- 冒险前置条件：下拉直选其他羁绊冒险 -->
                <select
                  v-else-if="
                    f.key === 'adventure_folder' &&
                    String(cond.type ?? '') === 'adventure_completed'
                  "
                  class="flex-1
                    min-w-0
                    border
                    border-white/[0.1]
                    rounded-md
                    bg-black/[0.25]
                    px-2
                    py-1.5
                    text-xs
                    text-white
                    transition-all
                    focus:outline-none
                    focus:border-[var(--accent-color)]"
                  :value="condField(cond, f.key)"
                  @change="(e) => onCondField(i, f.key, (e.target as HTMLSelectElement).value)"
                >
                  <option value="">{{ t('scriptEditor.configTab.notSet') }}</option>
                  <option
                    v-if="
                      condField(cond, f.key) &&
                      !adventureOptionValues.includes(condField(cond, f.key))
                    "
                    :value="condField(cond, f.key)"
                  >
                    {{ condField(cond, f.key) }}
                  </option>
                  <option
                    v-for="o in adventureOptions"
                    :key="o.value"
                    :value="o.value"
                  >
                    {{ o.label }}
                  </option>
                </select>
                <input
                  v-else
                  class="flex-1
                    min-w-0
                    border
                    border-white/[0.1]
                    rounded-md
                    bg-black/[0.25]
                    px-2
                    py-1.5
                    text-xs
                    text-white
                    transition-all
                    focus:outline-none
                    focus:border-[var(--accent-color)]"
                  :placeholder="unlockFieldPlaceholderOf(f)"
                  :value="condField(cond, f.key)"
                  @change="(e) => onCondField(i, f.key, (e.target as HTMLInputElement).value)"
                />
              </div>
            </div>
            <button
              class="mt-1
                rounded-lg
                border
                border-dashed
                border-white/15
                px-3
                py-1.5
                text-xs
                text-white/45
                transition-all
                hover:border-brand
                hover:text-brand"
              @click="addCondition"
            >
              ＋ {{ t('scriptEditor.configTab.addUnlockCondition') }}
            </button>
            <p class="mt-2
              text-[0.72rem]
              leading-[1.7]
              text-white/40">
              {{ t('scriptEditor.configTab.unlockTypesHint') }}
            </p>
          </div>
        </template>
      </div>

      <Button
        type="big"
        class="mt-4"
        @click="saveConfig"
      >
        {{ t('scriptEditor.configTab.save') }}
      </Button>
    </MenuItem>
  </MenuPage>
</template>
