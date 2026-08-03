<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { Button, Icon, Toggle } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import type { UnlockConditionSpec } from '@/api/services/script-editor'

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

const unlockSpecs = computed<UnlockConditionSpec[]>(
  () => store.schema?.unlockConditionTypes ?? [],
)

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
    <MenuItem title="剧本设置">
      <template #header>
        <Icon
          icon="setting"
          :size="20"
        />
      </template>

      <p class="mb-[0.9rem] rounded-xl border border-white/10 bg-black/16 px-[0.85rem] py-[0.7rem] text-[0.76rem] leading-[1.85] text-white/60">
        改写 <code class="font-mono text-brand">story_config.yaml</code> 会丢掉文件里的 YAML 注释（六个官方剧本的
        config 都带中文注释）。保存前会自动留一份 <code class="font-mono text-brand">.bak</code>。
      </p>

      <div
        v-for="f in store.schema?.storyConfigFields ?? []"
        :key="f.key"
        class="mb-4"
      >
        <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">
          {{ f.label }}<span
            v-if="f.required"
            class="ml-0.5 text-[0.7rem] text-red-400"
            >＊</span
          >
        </label>
        <p class="my-1 mb-2 text-[0.8rem] text-gray-300">{{ f.key }}</p>
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
          class="glass-input min-h-16"
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
          class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40 [&_code]:font-mono [&_code]:text-brand"
        >
          {{ f.hint }}
        </p>
      </div>

      <!-- 羁绊冒险 -->
      <div class="my-4 rounded-xl border border-white/10 bg-black/15 p-4">
        <label class="inline-flex items-center gap-2 text-[0.8rem] whitespace-nowrap text-white/70 mb-2">
          <Toggle
            :checked="isAdventure"
            @change="toggleAdventure"
          />
          这是某个角色的羁绊冒险
        </label>
        <template v-if="isAdventure">
          <div class="mb-4">
            <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">绑定角色目录名</label>
            <p class="my-1 mb-2 text-[0.8rem] text-gray-300">adventure.bound_character_folder</p>
            <input
              class="glass-input"
              :value="adventureField('bound_character_folder')"
              @change="(e) => onAdventureText('bound_character_folder', e)"
            />
          </div>
          <div class="mb-4">
            <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">排序</label>
            <p class="my-1 mb-2 text-[0.8rem] text-gray-300">adventure.order</p>
            <input
              class="glass-input"
              type="number"
              :value="adventureField('order')"
              @change="(e) => onAdventureNumber('order', e)"
            />
            <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40">数值越小越靠前显示，决定羁绊冒险在角色卡上的排列顺序</p>
          </div>

          <!-- 解锁条件：可视化编辑 -->
          <div class="mb-4">
            <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">解锁条件</label>
            <p class="my-1 mb-2 text-[0.8rem] text-gray-300">adventure.unlock_conditions</p>
            <p class="mb-2 text-[0.72rem] leading-[1.7] text-white/40">
              玩家要满足<b class="font-semibold text-white/80">全部</b>条件，冒险才会在角色卡上解锁；不设条件则默认一直可见。
            </p>
            <div
              v-for="(cond, i) in conditions"
              :key="i"
              class="mb-2 rounded-lg bg-white/6 p-2.5"
            >
              <div class="flex items-center gap-2">
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
                    {{ s.label }}
                  </option>
                </select>
                <button
                  class="shrink-0 rounded-md px-1.5 py-1 text-xs text-white/[0.35] transition-all hover:text-[#fca5a5] hover:bg-[rgba(248,113,113,0.15)]"
                  title="删除这个条件"
                  @click="removeCondition(i)"
                >
                  ✕
                </button>
              </div>
              <div
                v-for="f in unlockSpecByType.get(String(cond.type ?? ''))?.fields ?? []"
                :key="f.key"
                class="mt-2 flex items-center gap-2 pl-6"
              >
                <span class="shrink-0 text-xs text-white/40">{{ f.label }}</span>
                <input
                  v-if="f.kind === 'number'"
                  class="w-24 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
                  type="number"
                  :value="condField(cond, f.key)"
                  @change="(e) => onCondNumber(i, f.key, e)"
                />
                <input
                  v-else
                  class="flex-1 min-w-0 border border-white/[0.1] rounded-md bg-black/[0.25] px-2 py-1.5 text-xs text-white transition-all focus:outline-none focus:border-[var(--accent-color)]"
                  :placeholder="f.placeholder ?? f.hint"
                  :value="condField(cond, f.key)"
                  @change="(e) => onCondField(i, f.key, (e.target as HTMLInputElement).value)"
                />
              </div>
            </div>
            <button
              class="mt-1 rounded-lg border border-dashed border-white/15 px-3 py-1.5 text-xs text-white/45 transition-all hover:border-brand hover:text-brand"
              @click="addCondition"
            >
              ＋ 添加解锁条件
            </button>
            <p class="mt-2 text-[0.72rem] leading-[1.7] text-white/40">
              支持的类型：累计聊天条数 / 处于时间段内 / 已完成某个羁绊冒险 / 已解锁某个成就。
              <code class="font-mono text-brand">trigger.mode</code> 是旧版配置，引擎不读取，会原样保留。
            </p>
          </div>
        </template>
      </div>

      <Button
        type="big"
        class="mt-4"
        @click="saveConfig"
      >
        保存剧本设置
      </Button>
    </MenuItem>
  </MenuPage>
</template>
