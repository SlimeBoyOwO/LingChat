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
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import Icon from '@/components/base/widget/Icon.vue'
import { Toggle } from '@/components/base'
import { i18n } from '@/locales'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import {
  listPlugins,
  setPluginEnabled,
  savePluginConfig,
  deletePlugin,
  type PluginInfo,
} from '@/api/services/plugins'

const plugins = ref<PluginInfo[]>([])
const error = ref('')
const saving = ref(false)
const formState = reactive<Record<string, Record<string, unknown>>>({})
const dialogStore = useDialogStore()

const load = async () => {
  try {
    plugins.value = await listPlugins()
    for (const plugin of plugins.value) {
      if (!formState[plugin.id]) {
        formState[plugin.id] = {}
      }
    }
  } catch (e) {
    error.value = String(e)
  }
}

const toggle = async (plugin: PluginInfo, enabled: boolean) => {
  if (plugin.error) return
  try {
    await setPluginEnabled(plugin.id, enabled)
    plugin.enabled = enabled
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

onMounted(load)
</script>
