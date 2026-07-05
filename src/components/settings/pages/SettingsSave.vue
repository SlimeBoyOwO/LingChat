<template>
  <MenuPage>
    <MenuItem title="创建新存档（会记录当前对话）">
      <template #header>
        <PencilLine :size="20" />
      </template>
      <div class="flex gap-2.5">
        <Input
          type="text"
          v-model="newSaveTitle"
          placeholder="输入存档名称"
          @keyup.enter="handleCreateSave"
        />
        <button
          class="px-4 py-2 text-[#ddd] cursor-pointer rounded-md transition-all duration-200 whitespace-nowrap bg-[rgba(0,255,55,0.2)] border border-[rgba(0,255,55,0.3)] w-[10%] min-w-[65px] hover:-translate-y-px hover:bg-[rgba(0,255,55,0.35)] hover:shadow-[0_0_10px_rgba(0,255,55,0.15)]"
          @click="handleCreateSave"
          :disabled="actionLoading !== null"
        >
          {{ actionLoading === -1 ? '创建中...' : '创建' }}
        </button>
      </div>
    </MenuItem>
    <MenuItem title="存档列表">
      <template #header>
        <LayoutList :size="20" />
      </template>
      <div class="flex flex-col">
        <div class="h-[calc(100vh-22rem)] min-h-[300px] overflow-y-auto pr-1 pb-4">
          <div v-if="loading" class="text-center text-[#888] p-8">加载中...</div>

          <div v-else-if="error" class="text-center text-[#ff6b6b] p-8">加载失败: {{ error }}</div>

          <div v-else-if="saves.length === 0" class="text-center text-[#888] p-8">暂无存档记录</div>

          <div v-else class="grid grid-cols-1 md:grid-cols-2 gap-5 p-[5px]">
            <div
              v-for="(save, index) in saves"
              :key="save.id"
              class="bg-[rgba(20,20,20,0.45)] border border-white/10 rounded-xl p-4 flex flex-col transition-all duration-300 ease-[cubic-bezier(0.25,0.8,0.25,1)] shadow-[0_8px_32px_rgba(0,0,0,0.2)] backdrop-blur-[10px] hover:-translate-y-[3px] hover:border-[rgba(121,217,255,0.35)] hover:shadow-[0_12px_40px_rgba(121,217,255,0.08)] hover:bg-[rgba(20,20,20,0.55)]"
            >
              <div class="flex gap-4">
                <!-- Left: Screenshot Preview -->
                <div
                  class="w-1/2 h-48 rounded-lg overflow-hidden bg-black/40 border border-white/[0.08] shrink-0"
                >
                  <img
                    v-if="save.screenshot"
                    :src="`${convertFileSrc(save.screenshot)}?v=${save.update_date}`"
                    class="w-full h-full object-cover animate-[fadeIn_0.4s_ease]"
                    alt="game screenshot"
                  />
                  <div
                    v-else
                    class="w-full h-full bg-white/[0.02] flex flex-col items-center justify-center"
                  >
                    <SaveIcon :size="24" class="text-white/20 mb-1" />
                    <span class="text-[10px] text-white/30 font-semibold">暂无截图</span>
                  </div>
                </div>

                <!-- Right: Save Info -->
                <div class="flex-1 flex flex-col justify-between overflow-hidden">
                  <!-- Line 1: Index & Time -->
                  <div class="flex justify-between items-center text-xs text-white/40 font-mono">
                    <span class="font-bold">No.{{ index + 1 }}</span>
                    <span class="flex items-center gap-1">
                      <Clock :size="10" />
                      {{ formatDate(save.update_date) }}
                    </span>
                  </div>

                  <!-- Line 2: Title (Editable on Double Click) -->
                  <div class="mt-1.5 min-h-[26px] flex items-center">
                    <input
                      v-if="editingSaveId === save.id"
                      v-model="editTitleText"
                      v-focus
                      @blur="handleSaveTitle(save.id)"
                      @keyup.enter="handleSaveTitle(save.id)"
                      class="bg-black/50 border border-[rgba(121,217,255,0.5)] text-white text-sm font-bold rounded px-1.5 py-0.5 w-full outline-none"
                    />
                    <div
                      v-else
                      @dblclick="startEditTitle(save)"
                      class="text-sm font-bold max-w-full select-none cursor-pointer text-white hover:text-sky-300 transition-colors duration-200 truncate"
                      title="双击以修改存档标题"
                    >
                      {{ save.title || '未命名存档' }}
                    </div>
                  </div>

                  <!-- Separator -->
                  <div class="border-b border-dashed border-white/15 my-2 w-full"></div>

                  <!-- Line 3: Last Message -->
                  <div
                    class="text-xs text-white/65 leading-[1.4] h-[33px] italic line-clamp-2"
                    :title="save.last_message"
                  >
                    {{ save.last_message || '暂无对话台词记录' }}
                  </div>
                </div>
              </div>

              <!-- Bottom: Buttons -->
              <div class="mt-4 flex gap-2 pt-3 border-t border-white/5">
                <button
                  @click="handleLoadSave(save.id)"
                  class="px-3 py-1.5 rounded-md text-xs font-semibold cursor-pointer transition-all duration-200 text-white whitespace-nowrap bg-blue-500/25 border border-blue-500/40 hover:bg-blue-500/45 hover:shadow-[0_0_10px_rgba(59,130,246,0.2)] disabled:opacity-50 disabled:cursor-not-allowed flex-1"
                  :disabled="actionLoading !== null"
                >
                  {{ actionLoading === save.id ? '读取中...' : '读取存档' }}
                </button>
                <button
                  @click="handleSaveGame(save.id)"
                  class="px-3 py-1.5 rounded-md text-xs font-semibold cursor-pointer transition-all duration-200 text-white whitespace-nowrap bg-emerald-500/25 border border-emerald-500/40 hover:bg-emerald-500/45 hover:shadow-[0_0_10px_rgba(16,185,129,0.2)] disabled:opacity-50 disabled:cursor-not-allowed flex-1"
                  :disabled="actionLoading !== null"
                >
                  {{ actionLoading === save.id ? '保存中...' : '覆盖存档' }}
                </button>
                <button
                  @click="handleDeleteSave(save.id)"
                  class="px-3 py-1.5 rounded-md text-xs font-semibold cursor-pointer transition-all duration-200 text-white whitespace-nowrap bg-red-500/25 border border-red-500/40 hover:bg-red-500/45 hover:shadow-[0_0_10px_rgba(239,68,68,0.2)] disabled:opacity-50 disabled:cursor-not-allowed flex-1"
                  :disabled="actionLoading !== null"
                >
                  {{ actionLoading === save.id ? '删除中...' : '删除存档' }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { Input } from '../../base'
import { useGameStore } from '../../../stores/modules/game'
import { applyWebInitData } from '../../../stores/modules/game/actions'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useDialogStore } from '../../../stores/modules/ui/dialog'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import type { SaveInfo } from '../../../types'
import type { WebInitData } from '../../../api/services/game-info'
import { Save as SaveIcon, PencilLine, LayoutList, Clock } from 'lucide-vue-next'

interface SaveListResponse {
  saves: SaveInfo[]
  total: number
}

interface CreateSaveResponse {
  save_id: number
  message: string
}

const gameStore = useGameStore()
const uiStore = useUIStore()
const dialogStore = useDialogStore()

const saves = ref<SaveInfo[]>([])
const newSaveTitle = ref('')
const loading = ref(false)
const error = ref<string | null>(null)
const actionLoading = ref<number | null>(null)

// Title editing state
const editingSaveId = ref<number | null>(null)
const editTitleText = ref('')

// Custom directive for input auto-focus
const vFocus = {
  mounted: (el: HTMLInputElement) => el.focus(),
}

const startEditTitle = (save: SaveInfo) => {
  editingSaveId.value = save.id
  editTitleText.value = save.title
}

const handleSaveTitle = async (saveId: number) => {
  const newTitle = editTitleText.value.trim()
  if (!newTitle) {
    uiStore.showWarning({ title: '提示', message: '存档名称不能为空' })
    editingSaveId.value = null
    return
  }

  const save = saves.value.find((s) => s.id === saveId)
  if (save && save.title === newTitle) {
    editingSaveId.value = null
    return
  }

  try {
    await invoke('update_save_title', { saveId, title: newTitle })
    if (save) {
      save.title = newTitle
    }
    uiStore.showSuccess({ title: '修改成功', message: '存档名称已修改' })
  } catch (e: any) {
    console.error('修改存档名称失败:', e)
    uiStore.showError({
      title: '修改失败',
      message: typeof e === 'string' ? e : e.message || '未知错误',
    })
  } finally {
    editingSaveId.value = null
  }
}

const formatDate = (dateString: string): string => {
  const date = new Date(dateString)
  const pad = (n: number) => n.toString().padStart(2, '0')
  return `${date.getFullYear()}.${pad(date.getMonth() + 1)}.${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`
}

const fetchSaves = async () => {
  loading.value = true
  error.value = null
  try {
    const result = await invoke<SaveListResponse>('list_saves', {
      page: 1,
      pageSize: 50,
    })
    saves.value = result.saves
  } catch (e: any) {
    console.error('获取存档列表失败:', e)
    error.value = typeof e === 'string' ? e : e.message || '未知错误'
  } finally {
    loading.value = false
  }
}

/** 确保截图已就绪：若最新截图为空但仍有进行中的截图任务，等待它完成。 */
const ensureScreenshot = async (): Promise<string | null> => {
  if (gameStore.latestScreenshot) return gameStore.latestScreenshot
  if (gameStore.screenshotPending) {
    await gameStore.screenshotPending
  }
  return gameStore.latestScreenshot
}

const handleCreateSave = async () => {
  if (!newSaveTitle.value.trim()) {
    uiStore.showWarning({ title: '提示', message: '请输入存档名称' })
    return
  }
  actionLoading.value = -1
  try {
    await invoke<CreateSaveResponse>('create_save', {
      title: newSaveTitle.value.trim(),
      screenshotPath: await ensureScreenshot(),
    })
    newSaveTitle.value = ''
    uiStore.showSuccess({ title: '创建成功', message: '存档已创建' })
    await fetchSaves()
  } catch (e: any) {
    console.error('创建存档失败:', e)
    uiStore.showError({
      title: '创建失败',
      message: typeof e === 'string' ? e : e.message || '未知错误',
    })
  } finally {
    actionLoading.value = null
  }
}

const handleLoadSave = async (saveId: number) => {
  const confirmed = await dialogStore.confirm('加载存档会导致丢失当前对话进度，确定要加载吗？')
  if (!confirmed) return
  actionLoading.value = saveId
  try {
    const gameInfo = await invoke<WebInitData>('load_save', { saveId })
    applyWebInitData(gameStore.$state, gameInfo)
    uiStore.showSuccess({ title: '加载成功', message: '存档已加载' })
  } catch (e: any) {
    console.error('读取存档失败:', e)
    uiStore.showError({
      title: '加载失败',
      message: typeof e === 'string' ? e : e.message || '未知错误',
    })
  } finally {
    actionLoading.value = null
  }
}

const handleSaveGame = async (saveId: number) => {
  const confirmed = await dialogStore.confirm('覆盖存档会导致丢失之前的存档进度，确定要覆盖吗？')
  if (!confirmed) return
  actionLoading.value = saveId
  try {
    await invoke('update_save', {
      saveId,
      screenshotPath: await ensureScreenshot(),
    })
    uiStore.showSuccess({ title: '保存成功', message: '存档已覆盖' })
    await fetchSaves()
  } catch (e: any) {
    console.error('保存游戏失败:', e)
    uiStore.showError({
      title: '保存失败',
      message: typeof e === 'string' ? e : e.message || '未知错误',
    })
  } finally {
    actionLoading.value = null
  }
}

const handleDeleteSave = async (saveId: number) => {
  if (!(await dialogStore.confirm('确定要删除这个存档吗？此操作不可撤销。'))) return
  actionLoading.value = saveId
  try {
    await invoke('delete_save', { saveId })
    uiStore.showSuccess({ title: '删除成功', message: '存档已删除' })
    await fetchSaves()
  } catch (e: any) {
    console.error('删除存档失败:', e)
    uiStore.showError({
      title: '删除失败',
      message: typeof e === 'string' ? e : e.message || '未知错误',
    })
  } finally {
    actionLoading.value = null
  }
}

onMounted(() => {
  fetchSaves()
})
</script>

<style scoped>
@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}
</style>
