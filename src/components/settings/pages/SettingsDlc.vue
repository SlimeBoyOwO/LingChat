<template>
  <div class="h-full overflow-y-auto p-2 select-none">
    <!-- 安全提示：DLC 是第三方剧本包，内容未经审核 -->
    <div
      class="mb-4 flex items-start gap-2 rounded-lg border border-amber-300/20 bg-amber-400/10 px-4 py-3 text-sm leading-relaxed text-amber-100/80"
    >
      <AlertTriangle :size="16" class="mt-0.5 shrink-0" />
      <span>{{ $t('advance.dlc.notice') }}</span>
    </div>

    <!-- 添加按钮 -->
    <div class="mb-5">
      <Button type="big" icon="advance" :icon_size="18" :disabled="importing" @click="pickAndImport">
        {{ importing ? $t('advance.dlc.importing') : $t('advance.dlc.add') }}
      </Button>
    </div>

    <!-- DLC 列表 -->
    <div v-if="dlcs.length === 0" class="py-10 text-center text-sm text-white/40">
      {{ $t('advance.dlc.empty') }}
    </div>

    <div v-else class="flex flex-col gap-3">
      <div
        v-for="dlc in dlcs"
        :key="dlc.folder_key"
        class="rounded-lg border border-white/10 bg-white/5 px-4 py-3 backdrop-blur-sm"
      >
        <div class="flex items-center justify-between gap-3">
          <div class="flex min-w-0 items-center gap-2">
            <Package :size="16" class="shrink-0 text-white/60" />
            <span class="truncate font-medium text-white/90">{{ dlc.name }}</span>
            <span v-if="dlc.version" class="shrink-0 text-xs text-white/40">v{{ dlc.version }}</span>
            <span
              v-if="dlc.content_warning === 'horror'"
              class="shrink-0 rounded border border-red-400/40 bg-red-500/15 px-1.5 py-0.5 text-xs text-red-300"
            >
              {{ $t('advance.dlc.warningHorror') }}
            </span>
          </div>
          <button
            type="button"
            class="shrink-0 rounded-md border border-white/15 px-3 py-1 text-xs text-white/60 transition-colors hover:border-red-300/40 hover:text-red-300"
            :disabled="removing !== null"
            @click="confirmRemove(dlc)"
          >
            {{ $t('advance.dlc.remove') }}
          </button>
        </div>
        <p v-if="dlc.description" class="mt-1.5 line-clamp-2 text-xs leading-relaxed text-white/45">
          {{ dlc.description }}
        </p>
        <p v-if="dlc.author" class="mt-1 text-xs text-white/30">{{ dlc.author }}</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { AlertTriangle, Package } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { Button } from '../../base'
import { useDialogStore } from '@/stores/modules/ui/dialog'
import { useUIStore } from '@/stores/modules/ui/ui'
import { importDlc, listDlcs, removeDlc, type DlcInfo } from '@/api/services/dlc'

const { t } = useI18n()
const dialogStore = useDialogStore()
const uiStore = useUIStore()

const dlcs = ref<DlcInfo[]>([])
const importing = ref(false)
const removing = ref<string | null>(null)

async function refresh() {
  try {
    dlcs.value = await listDlcs()
  } catch (e) {
    console.warn('[DLC] 列表读取失败:', e)
  }
}

async function pickAndImport() {
  if (importing.value) return
  const selected = await open({
    multiple: false,
    filters: [{ name: 'LingChat DLC', extensions: ['zip'] }],
  })
  if (!selected || typeof selected !== 'string') return

  importing.value = true
  try {
    await importDlc(selected)
    await refresh()
    // 通知主菜单等刷新剧本列表 / DLC 提示
    uiStore.dlcRefreshToken += 1
  } catch (e) {
    await dialogStore.alert(String(e), t('advance.dlc.add'))
  } finally {
    importing.value = false
  }
}

async function confirmRemove(dlc: DlcInfo) {
  if (removing.value) return
  const confirmed = await dialogStore.confirm(
    t('advance.dlc.removeConfirm', { name: dlc.name }),
    t('advance.dlc.remove'),
  )
  if (!confirmed) return

  removing.value = dlc.folder_key
  try {
    await removeDlc(dlc.folder_key)
    await refresh()
    uiStore.dlcRefreshToken += 1
  } catch (e) {
    await dialogStore.alert(String(e), t('advance.dlc.remove'))
  } finally {
    removing.value = null
  }
}

onMounted(refresh)
</script>
