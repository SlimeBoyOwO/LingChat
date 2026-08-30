<template>
  <MenuPage>
    <MenuItem :title="$t('settings.plugins.title')">
      <template #header>
        <Icon icon="package" :size="20" />
      </template>

      <!-- 错误提示 -->
      <div
        v-if="error"
        class="mb-4 px-4 py-2.5 rounded-xl border border-red-500/40 bg-red-500/10 text-red-200 text-sm"
      >
        {{ error }}
      </div>

      <div class="space-y-4">
        <div
          v-for="plugin in plugins"
          :key="plugin.id"
          class="rounded-xl border border-white/10 bg-white/5 backdrop-blur-md p-4"
        >
          <!-- 头部：名称 + 版本 + 开关 -->
          <div class="flex items-center justify-between gap-3">
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <h3 class="text-base font-bold text-white truncate">{{ plugin.name }}</h3>
                <span
                  class="shrink-0 text-[10px] px-2 py-0.5 rounded-full border border-white/10 text-white/60"
                >
                  v{{ plugin.version }}
                </span>
              </div>
              <p class="text-xs text-white/60 mt-0.5">{{ plugin.description }}</p>
            </div>
            <Toggle
              class="shrink-0"
              :checked="plugin.enabled"
              :disabled="!!plugin.error"
              @change="(v: boolean) => toggle(plugin, v)"
            />
          </div>

          <!-- 错误信息 -->
          <p v-if="plugin.error" class="mt-2 text-xs text-red-300">{{ plugin.error }}</p>

          <!-- 工具列表 -->
          <div v-if="plugin.tools.length" class="mt-3 flex flex-wrap gap-1.5">
            <span
              v-for="tool in plugin.tools"
              :key="tool"
              class="text-[11px] px-2 py-0.5 rounded-md bg-white/5 border border-white/10 text-white/70 font-mono"
            >
              {{ tool }}
            </span>
          </div>

          <!-- 携带资源区 -->
          <div v-if="plugin.resources.length" class="mt-3">
            <button
              type="button"
              class="flex items-center gap-1.5 text-[11px] text-white/60 hover:text-white/90 transition-colors"
              @click="toggleResourcePanel(plugin.id)"
            >
              <ChevronDown
                :size="13"
                class="transition-transform"
                :class="expandedResources[plugin.id] ? '' : '-rotate-90'"
              />
              {{ $t('settings.plugins.resourcesTitle') }}（{{
                plugin.resources.map((k) => kindLabel(k as ResourceKind)).join(' / ')
              }}）
            </button>

            <div v-if="expandedResources[plugin.id]" class="mt-2 space-y-1.5">
              <p v-if="!resourcesOf(plugin.id).length" class="text-[11px] text-white/40 pl-4">
                {{ $t('settings.plugins.resourcesEmpty') }}
              </p>
              <div
                v-for="res in resourcesOf(plugin.id)"
                :key="res.kind + '/' + res.key"
                class="flex items-center gap-2 pl-4 text-xs"
              >
                <span
                  class="shrink-0 text-[10px] px-1.5 py-0.5 rounded bg-white/5 border border-white/10 text-white/50"
                >
                  {{ kindLabel(res.kind) }}
                </span>
                <span class="min-w-0 flex-1 truncate text-white/80">{{ res.name }}</span>
                <span
                  v-if="res.conflict"
                  class="shrink-0 text-[10px] text-amber-300"
                  :title="$t('settings.plugins.resourceConflictHint')"
                >
                  {{ $t('settings.plugins.resourceConflict') }}
                </span>
                <span v-else-if="res.hidden" class="shrink-0 text-[10px] text-white/40">
                  {{ $t('settings.plugins.resourceHidden') }}
                </span>

                <template v-if="res.hidden">
                  <button
                    type="button"
                    class="shrink-0 px-2 py-0.5 rounded-md bg-white/5 border border-white/10 text-white/70 text-[11px] hover:bg-white/10 transition-colors"
                    @click="restoreResource(plugin.id, res)"
                  >
                    {{ $t('settings.plugins.resourceRestore') }}
                  </button>
                </template>
                <template v-else>
                  <button
                    v-if="!res.conflict"
                    type="button"
                    class="shrink-0 px-2 py-0.5 rounded-md bg-brand/70 text-white text-[11px] hover:bg-brand transition-colors"
                    :title="$t('settings.plugins.resourceKeepHint')"
                    @click="keepResource(plugin.id, res)"
                  >
                    {{ $t('settings.plugins.resourceKeep') }}
                  </button>
                  <button
                    type="button"
                    class="shrink-0 px-2 py-0.5 rounded-md bg-white/5 border border-white/10 text-white/70 text-[11px] hover:bg-white/10 transition-colors"
                    @click="hideResource(plugin.id, res)"
                  >
                    {{ $t('settings.plugins.resourceHide') }}
                  </button>
                </template>
              </div>
            </div>
          </div>

          <!-- 环境变量提示 -->
          <div v-if="plugin.env.length" class="mt-3">
            <p class="text-[11px] text-white/50 mb-1">{{ $t('settings.plugins.envHint') }}</p>
            <div v-for="env in plugin.env" :key="env.key" class="flex items-center gap-2">
              <span class="text-xs font-mono text-white/80">{{ env.key }}</span>
              <span
                class="text-[10px] px-1.5 py-0.5 rounded bg-white/5 border border-white/10 text-white/50"
              >
                {{ $t('settings.plugins.envFromProcess') }}
              </span>
            </div>
          </div>

          <!-- 配置表单 -->
          <div v-if="plugin.config_schema.length" class="mt-3 space-y-2.5">
            <div
              v-for="field in plugin.config_schema"
              :key="field.key"
              class="flex items-center gap-2"
            >
              <label class="text-xs text-white/70 w-28 shrink-0">{{ field.label }}</label>
              <input
                v-if="field.kind === 'boolean'"
                type="checkbox"
                class="accent-brand"
                :checked="(formState[plugin.id]?.[field.key] as boolean) === true"
                @change="onBoolChange(plugin, field.key, ($event.target as HTMLInputElement).checked)"
              />
              <input
                v-else
                :type="field.kind === 'secret' ? 'password' : field.kind === 'number' ? 'number' : 'text'"
                class="flex-1 min-w-0 px-3 py-1.5 rounded-lg bg-white/5 border border-white/10 text-white text-sm focus:outline-none focus:border-brand/60"
                :value="formState[plugin.id]?.[field.key] ?? ''"
                @input="onInput(plugin, field.key, ($event.target as HTMLInputElement).value)"
              />
            </div>
            <div class="flex justify-end">
              <button
                type="button"
                class="px-3 py-1.5 rounded-lg bg-brand/70 text-white text-xs hover:bg-brand transition-colors"
                :disabled="saving"
                @click="saveConfig(plugin)"
              >
                {{ $t('settings.plugins.saveConfig') }}
              </button>
            </div>
          </div>

          <!-- 删除 -->
          <div class="mt-3 flex justify-end">
            <button
              type="button"
              class="flex items-center gap-1 px-3 py-1.5 rounded-lg bg-red-500/10 border border-red-500/30 text-red-300 text-xs hover:bg-red-500/20 transition-colors"
              @click="removePlugin(plugin)"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="M3 6h18" />
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
                <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                <line x1="10" x2="10" y1="11" y2="17" />
                <line x1="14" x2="14" y1="11" y2="17" />
              </svg>
              {{ $t('settings.plugins.delete') }}
            </button>
          </div>
        </div>

        <p v-if="!plugins.length" class="text-sm text-white/50 text-center py-8">
          {{ $t('settings.plugins.empty') }}
        </p>
      </div>
    </MenuItem>

    <PluginArchiveProgress />

    <MenuItem :title="$t('settings.plugins.import.title')" size="small">
      <template #header>
        <PackageOpen :size="20" />
      </template>
      <div class="space-y-2">
        <div class="flex flex-col gap-1.5">
          <label class="text-xs text-white/60 font-medium">{{
            $t('settings.plugins.import.conflictPolicy')
          }}</label>
          <select
            v-model="conflictPolicy"
            class="bg-black/20 border border-white/10 rounded-xl px-3 py-2 text-white text-sm outline-none transition-all duration-200"
          >
            <option value="overwrite">{{ $t('settings.plugins.import.policyOverwrite') }}</option>
            <option value="abort">{{ $t('settings.plugins.import.policyAbort') }}</option>
          </select>
        </div>
        <Button type="big" @click="handleImport">{{ $t('settings.plugins.import.button') }}</Button>
        <p class="text-[11px] text-white/40 leading-relaxed">
          {{ $t('settings.plugins.import.hint') }}
        </p>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { ChevronDown, PackageOpen } from 'lucide-vue-next'
import { MenuPage, MenuItem } from '../../ui'
import { Button } from '../../base'
import Icon from '@/components/base/widget/Icon.vue'
import { Toggle } from '@/components/base'
import { i18n } from '@/locales'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import PluginArchiveProgress from '@/components/ui/PluginArchiveProgress.vue'
import { usePluginImport } from '@/composables/usePluginImport'
import type { PluginConflictPolicy } from '@/api/services/plugins'
import {
  listPlugins,
  setPluginEnabled,
  savePluginConfig,
  deletePlugin,
  pluginResources,
  pluginResourceHide,
  pluginResourceRestore,
  pluginResourceKeep,
  type PluginInfo,
  type PluginResourceEntry,
  type ResourceKind,
} from '@/api/services/plugins'

const plugins = ref<PluginInfo[]>([])
const error = ref('')
const saving = ref(false)
const formState = reactive<Record<string, Record<string, unknown>>>({})
const dialogStore = useDialogStore()

// 每个插件的资源条目 + 展开状态（懒加载：只在展开或声明资源时拉取）
const resourceMap = reactive<Record<string, PluginResourceEntry[]>>({})
const expandedResources = reactive<Record<string, boolean>>({})

const resourcesOf = (id: string): PluginResourceEntry[] => resourceMap[id] ?? []

const kindLabel = (kind: ResourceKind): string =>
  i18n.global.t(`settings.plugins.resourceKinds.${kind}`)

const loadResources = async (id: string) => {
  try {
    resourceMap[id] = await pluginResources(id)
  } catch (e) {
    error.value = String(e)
  }
}

const toggleResourcePanel = async (id: string) => {
  expandedResources[id] = !expandedResources[id]
  if (expandedResources[id] && !resourceMap[id]) {
    await loadResources(id)
  }
}

const refreshAfter = async (id: string) => {
  await loadResources(id)
  await load()
}

const hideResource = async (id: string, res: PluginResourceEntry) => {
  try {
    await pluginResourceHide(id, `${res.kind}/${res.key}`)
    await refreshAfter(id)
  } catch (e) {
    error.value = String(e)
  }
}

const restoreResource = async (id: string, res: PluginResourceEntry) => {
  try {
    await pluginResourceRestore(id, `${res.kind}/${res.key}`)
    await refreshAfter(id)
  } catch (e) {
    error.value = String(e)
  }
}

const keepResource = async (id: string, res: PluginResourceEntry) => {
  const confirmed = await dialogStore.confirm(
    i18n.global.t('settings.plugins.resourceKeepConfirm', {
      name: res.name,
      kind: kindLabel(res.kind),
    }),
  )
  if (!confirmed) return
  try {
    await pluginResourceKeep(id, `${res.kind}/${res.key}`)
    await refreshAfter(id)
  } catch (e) {
    error.value = String(e)
  }
}

const load = async () => {
  try {
    plugins.value = await listPlugins()
    for (const plugin of plugins.value) {
      if (!formState[plugin.id]) {
        formState[plugin.id] = {}
      }
      // 声明了资源且面板已展开的，刷新条目
      if (plugin.resources.length && expandedResources[plugin.id]) {
        await loadResources(plugin.id)
      }
    }
  } catch (e) {
    error.value = String(e)
  }
}

const toggle = async (plugin: PluginInfo, enabled: boolean) => {
  if (plugin.error) return
  // 禁用会移除插件角色并级联删除其存档/记忆（重启用不恢复），破坏性操作前确认。
  if (!enabled && plugin.resources.includes('characters')) {
    const ok = await dialogStore.confirm(
      i18n.global.t('settings.plugins.disableCharactersConfirm', { name: plugin.name }),
    )
    if (!ok) return
  }
  try {
    await setPluginEnabled(plugin.id, enabled)
    plugin.enabled = enabled
    if (plugin.resources.length) {
      await loadResources(plugin.id)
    }
  } catch (e) {
    error.value = String(e)
  }
}

const onInput = (plugin: PluginInfo, key: string, value: string) => {
  formState[plugin.id][key] = value
}

const onBoolChange = (plugin: PluginInfo, key: string, value: boolean) => {
  formState[plugin.id][key] = value
}

const saveConfig = async (plugin: PluginInfo) => {
  saving.value = true
  try {
    await savePluginConfig(plugin.id, formState[plugin.id] ?? {})
  } catch (e) {
    error.value = String(e)
  } finally {
    saving.value = false
  }
}

const removePlugin = async (plugin: PluginInfo) => {
  const confirmed = await dialogStore.confirm(
    i18n.global.t('settings.plugins.deleteConfirm', { name: plugin.name }),
  )
  if (!confirmed) return
  try {
    await deletePlugin(plugin.id)
    delete formState[plugin.id]
    await load()
  } catch (e) {
    error.value = String(e)
  }
}

// ===== 压缩包导入 =====

const { store: importStore, pickAndImport } = usePluginImport()
const conflictPolicy = ref<PluginConflictPolicy>('overwrite')

const handleImport = async () => {
  await pickAndImport(conflictPolicy.value)
  const result = importStore.import.result as { plugin_id?: string } | null
  // 覆盖导入会换掉整套资源文件，丢弃该插件的资源缓存，避免展开时显示旧条目。
  if (result?.plugin_id) {
    delete resourceMap[result.plugin_id]
    delete formState[result.plugin_id]
  }
  // 导入对话框关闭后（成功、失败或取消）都重新拉一次列表。
  await load()
}

onMounted(() => {
  load()
})
</script>
