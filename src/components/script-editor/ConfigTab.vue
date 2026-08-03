<script setup lang="ts">
import { computed, reactive, watch } from 'vue'
import { Button, Icon, Toggle } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'

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
          <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40 [&_code]:font-mono [&_code]:text-brand">
            解锁条件（<code class="font-mono text-brand">unlock_conditions</code>）目前保持文件里的原值不动，
            下一轮补可视化编辑。<code class="font-mono text-brand">trigger.mode</code> 引擎没有任何消费者，
            因此不在这里暴露，但读写时原样保留。
          </p>
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
