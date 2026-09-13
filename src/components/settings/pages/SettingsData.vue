<template>
  <MenuPage>
    <!-- ─── 导出 ─────────────────────────────────────────── -->
    <MenuItem :title="$t('settings.data.export.title')" size="large">
      <template #header>
        <Download :size="20" />
      </template>
      <div class="space-y-4">
        <p class="text-xs text-white/50">{{ $t('settings.data.export.hint') }}</p>

        <!-- 勾选列表 -->
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
          <label
            v-for="key in exportKeys"
            :key="key"
            class="flex items-center gap-2.5 px-3 py-2 rounded-lg bg-white/[0.03] border border-white/[0.06] cursor-pointer transition-all hover:bg-white/[0.06]"
            :class="{ 'opacity-40 pointer-events-none': exportRunning }"
          >
            <input
              type="checkbox"
              v-model="selections[key]"
              class="w-4 h-4 rounded accent-blue-500 shrink-0"
            />
            <span class="text-sm text-white/80">{{ $t(`settings.data.items.${key}`) }}</span>
          </label>
        </div>

        <!-- 全选 / 全不选 -->
        <div class="flex gap-2">
          <button
            class="text-xs px-3 py-1.5 rounded-md bg-white/[0.06] text-white/60 hover:bg-white/[0.1] transition-colors"
            :disabled="exportRunning"
            @click="selectAll"
          >
            {{ $t('settings.data.selectAll') }}
          </button>
          <button
            class="text-xs px-3 py-1.5 rounded-md bg-white/[0.06] text-white/60 hover:bg-white/[0.1] transition-colors"
            :disabled="exportRunning"
            @click="deselectAll"
          >
            {{ $t('settings.data.deselectAll') }}
          </button>
        </div>

        <!-- 操作按钮 -->
        <Button type="big" @click="handleExport" :disabled="exportRunning || !anySelected">
          <template #icon><Download :size="18" /></template>
          {{ exportRunning ? $t('settings.data.exporting') : $t('settings.data.export.button') }}
        </Button>

        <!-- 进度 / 结果 -->
        <div v-if="exportPhase !== 'idle'" class="mt-2">
          <div class="flex items-center justify-between text-xs text-white/50 mb-1">
            <span>{{ exportMessage }}</span>
            <span v-if="exportPercent >= 0">{{ exportPercent }}%</span>
          </div>
          <div class="h-1.5 rounded-full bg-white/[0.06] overflow-hidden">
            <div
              class="h-full rounded-full transition-all duration-300"
              :class="{
                'bg-blue-500': exportPhase === 'running',
                'bg-green-500': exportPhase === 'done',
                'bg-red-500': exportPhase === 'error',
                'bg-yellow-500': exportPhase === 'cancelled',
              }"
              :style="{ width: exportPercent >= 0 ? `${exportPercent}%` : '100%' }"
            />
          </div>
          <p v-if="exportPhase === 'done'" class="text-xs text-green-400 mt-1.5">
            ✓ {{ $t('settings.data.exportSuccess') }}
          </p>
          <p v-else-if="exportPhase === 'error'" class="text-xs text-red-400 mt-1.5">
            ✗ {{ exportError || $t('settings.data.exportFailed') }}
          </p>
        </div>
      </div>
    </MenuItem>

    <!-- ─── 导入 ─────────────────────────────────────────── -->
    <MenuItem :title="$t('settings.data.import.title')" size="large">
      <template #header>
        <Upload :size="20" />
      </template>
      <div class="space-y-4">
        <p class="text-xs text-white/50">{{ $t('settings.data.import.hint') }}</p>

        <!-- 选择文件 -->
        <Button type="big" @click="handlePickFile" :disabled="importRunning">
          <template #icon><FolderOpen :size="18" /></template>
          {{ $t('settings.data.import.pickFile') }}
        </Button>

        <!-- 已选文件信息 -->
        <div v-if="pickedFile" class="rounded-lg bg-white/[0.03] border border-white/[0.06] p-3 space-y-2">
          <div class="flex items-center gap-2 text-sm text-white/70">
            <FileArchive :size="16" class="shrink-0" />
            <span class="truncate">{{ pickedFile }}</span>
          </div>
          <div v-if="backupManifest" class="text-xs text-white/40 space-y-0.5">
            <div>{{ $t('settings.data.import.appVersion') }}: {{ backupManifest.appVersion }}</div>
            <div>{{ $t('settings.data.import.exportedAt') }}: {{ formatTimestamp(backupManifest.exportedAt) }}</div>
          </div>

          <!-- 可恢复项目勾选 -->
          <div v-if="backupManifest" class="pt-2 border-t border-white/[0.06] space-y-1.5">
            <p class="text-xs text-white/50 font-medium">{{ $t('settings.data.import.selectItems') }}</p>
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-1.5">
              <label
                v-for="key in importAvailableKeys"
                :key="key"
                class="flex items-center gap-2 px-2.5 py-1.5 rounded bg-white/[0.02] cursor-pointer hover:bg-white/[0.05] transition-colors"
              >
                <input
                  type="checkbox"
                  v-model="importSelections[key]"
                  class="w-3.5 h-3.5 rounded accent-blue-500 shrink-0"
                />
                <span class="text-xs text-white/70">{{ $t(`settings.data.items.${key}`) }}</span>
              </label>
            </div>
          </div>
        </div>

        <!-- 导入按钮 -->
        <Button
          v-if="pickedFile && backupManifest"
          type="big"
          @click="handleImport"
          :disabled="importRunning || !anyImportSelected"
        >
          <template #icon><Upload :size="18" /></template>
          {{ importRunning ? $t('settings.data.importing') : $t('settings.data.import.button') }}
        </Button>

        <!-- 进度 / 结果 -->
        <div v-if="importPhase !== 'idle'" class="mt-2">
          <div class="flex items-center justify-between text-xs text-white/50 mb-1">
            <span>{{ importMessage }}</span>
          </div>
          <div v-if="importPhase === 'done'" class="space-y-1 mt-2">
            <p class="text-xs text-green-400">✓ {{ $t('settings.data.importSuccess') }}</p>
            <div v-if="importResult" class="text-xs text-white/40 space-y-0.5">
              <div v-if="importResult.databaseImported">
                · {{ $t('settings.data.items.database') }}
              </div>
              <div v-if="importResult.settingsImported">
                · {{ $t('settings.data.items.settings') }}
              </div>
              <div v-if="importResult.filesRestored.length">
                · {{ importResult.filesRestored.map(k => $t(`settings.data.items.${k}`)).join(', ') }}
              </div>
            </div>
          </div>
          <p v-else-if="importPhase === 'error'" class="text-xs text-red-400 mt-1.5">
            ✗ {{ importError || $t('settings.data.importFailed') }}
          </p>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Download,
  FileArchive,
  FolderOpen,
  Upload,
} from 'lucide-vue-next'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { Button } from '../../base'
import { MenuItem, MenuPage } from '../../ui'
import { useSettingsStore } from '../../../stores/modules/settings'
import { relaunch } from '@tauri-apps/plugin-process'
import { useDialogStore } from '../../../stores/modules/ui/dialog'
import {
  ALL_SELECTIONS,
  exportDataBackup,
  importDataBackup,
  peekDataBackup,
  type BackupManifest,
  type BackupSelections,
  type ImportResult,
} from '../../../api/services/data-backup'

type DataKey = keyof BackupSelections

const { t } = useI18n()
const settingsStore = useSettingsStore()
const dialogStore = useDialogStore()

// 所有可勾选项
const exportKeys: DataKey[] = [
  'database',
  'settings',
  'frontendPreferences',
  'characters',
  'backgrounds',
  'musics',
  'ambients',
]

// ─── 导出状态 ────────────────────────────────────────────

const selections = ref<BackupSelections>({ ...ALL_SELECTIONS })
const exportPhase = ref<'idle' | 'running' | 'done' | 'error' | 'cancelled'>('idle')
const exportMessage = ref('')
const exportPercent = ref(-1)
const exportError = ref('')

const exportRunning = computed(() => exportPhase.value === 'running')
const anySelected = computed(() => Object.values(selections.value).some(Boolean))

function selectAll() {
  for (const k of exportKeys) selections.value[k] = true
}
function deselectAll() {
  for (const k of exportKeys) selections.value[k] = false
}

async function handleExport() {
  if (!anySelected.value) return

  exportPhase.value = 'running'
  exportMessage.value = t('settings.data.preparingExport')
  exportPercent.value = -1
  exportError.value = ''

  const ts = Date.now()
  const suggestedName = `lingchat_backup_${ts}.zip`

  let savedPath: string | null = null
  try {
    savedPath = await saveDialog({
      defaultPath: suggestedName,
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
    })
    if (!savedPath) {
      exportPhase.value = 'cancelled'
      exportMessage.value = t('settings.data.cancelled')
      return
    }

    exportMessage.value = t('settings.data.exportRunning')

    // 收集前端偏好 JSON
    const frontendPrefsJson = selections.value.frontendPreferences
      ? settingsStore.exportSettings()
      : null

    await exportDataBackup(selections.value, frontendPrefsJson, savedPath)

    exportPhase.value = 'done'
    exportPercent.value = 100
    exportMessage.value = t('settings.data.exportSuccess')
  } catch (e: any) {
    console.error('[DataBackup] export failed:', e)
    exportPhase.value = 'error'
    exportError.value = typeof e === 'string' ? e : e?.message || String(e)
    exportMessage.value = t('settings.data.exportFailed')
  }
}

// ─── 导入状态 ────────────────────────────────────────────

const pickedFile = ref<string | null>(null)
const backupManifest = ref<BackupManifest | null>(null)
const importSelections = ref<BackupSelections>({
  database: false,
  settings: false,
  frontendPreferences: false,
  characters: false,
  backgrounds: false,
  musics: false,
  ambients: false,
})
const importPhase = ref<'idle' | 'running' | 'done' | 'error' | 'cancelled'>('idle')
const importMessage = ref('')
const importError = ref('')
const importResult = ref<ImportResult | null>(null)

const importRunning = computed(() => importPhase.value === 'running')

// 备份中可用的项目
const importAvailableKeys = computed<DataKey[]>(() => {
  if (!backupManifest.value) return []
  const s = backupManifest.value.selections
  return exportKeys.filter(k => s[k])
})

const anyImportSelected = computed(() =>
  Object.values(importSelections.value).some(Boolean),
)

async function handlePickFile() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
    })
    if (!selected) return

    const path = typeof selected === 'string' ? selected : (selected as any).path
    if (!path) return

    pickedFile.value = path
    backupManifest.value = null
    importPhase.value = 'idle'
    importResult.value = null

    // 读取清单
    const manifest = await peekDataBackup(path)
    backupManifest.value = manifest

    // 默认勾选全部可用项
    const s = manifest.selections
    importSelections.value = {
      database: s.database,
      settings: s.settings,
      frontendPreferences: s.frontendPreferences,
      characters: s.characters,
      backgrounds: s.backgrounds,
      musics: s.musics,
      ambients: s.ambients,
    }
  } catch (e: any) {
    console.error('[DataBackup] peek failed:', e)
    importPhase.value = 'error'
    importError.value = typeof e === 'string' ? e : e?.message || String(e)
  }
}

async function handleImport() {
  if (!pickedFile.value || !anyImportSelected.value) return

  importPhase.value = 'running'
  importMessage.value = t('settings.data.importRunning')
  importError.value = ''
  importResult.value = null

  try {
    const result = await importDataBackup(pickedFile.value, importSelections.value)
    importResult.value = result

    // 恢复前端偏好
    if (result.frontendPreferencesJson) {
      settingsStore.importSettings(result.frontendPreferencesJson)
    }

    importPhase.value = 'done'
    importMessage.value = t('settings.data.importSuccess')

    // 恢复数据库或设置后，提示重启应用以加载新数据
    if (result.needsRestart) {
      const ok = await dialogStore.confirm(t('settings.data.restartHint'))
      if (ok) {
        try {
          await relaunch()
        } catch (e) {
          console.error('[DataBackup] restart failed:', e)
        }
      }
    }
  } catch (e: any) {
    console.error('[DataBackup] import failed:', e)
    importPhase.value = 'error'
    importError.value = typeof e === 'string' ? e : e?.message || String(e)
    importMessage.value = t('settings.data.importFailed')
  }
}

// ─── 辅助 ────────────────────────────────────────────────

function formatTimestamp(ms: number): string {
  return new Date(ms).toLocaleString()
}
</script>
