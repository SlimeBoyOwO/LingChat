<template>
  <MenuPage>
    <MenuItem title="创建新存档（会记录当前对话）">
      <template #header>
        <PencilLine :size="20" />
      </template>
      <div class="new-save-form">
        <Input
          type="text"
          v-model="newSaveTitle"
          placeholder="输入存档名称"
          @keyup.enter="handleCreateSave"
        />
        <button
          class="glass-effect action-btn-create"
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
      <div class="save-section">
        <div class="save-list-container">
          <div v-if="loading" class="status-message">加载中...</div>

          <div v-else-if="error" class="status-message error">加载失败: {{ error }}</div>

          <div v-else-if="saves.length === 0" class="status-message">暂无存档记录</div>

          <div v-else class="save-list">
            <div v-for="(save, index) in saves" :key="save.id" class="save-card-modern">
              <div class="save-card-top flex gap-4">
                <!-- Left: Screenshot Preview -->
                <div class="save-screenshot-container shrink-0">
                  <img
                    v-if="save.screenshot"
                    :src="convertFileSrc(save.screenshot)"
                    class="save-screenshot-img animate-fade-in"
                    alt="game screenshot"
                  />
                  <div
                    v-else
                    class="save-screenshot-placeholder flex flex-col items-center justify-center"
                  >
                    <SaveIcon :size="24" class="text-white/20 mb-1" />
                    <span class="text-[10px] text-white/30 font-semibold">暂无截图</span>
                  </div>
                </div>

                <!-- Right: Save Info -->
                <div class="save-meta flex-1 flex flex-col justify-between overflow-hidden">
                  <!-- Line 1: Index & Time -->
                  <div class="flex justify-between items-center text-xs text-white/40 font-mono">
                    <span class="save-index font-bold">No.{{ index + 1 }}</span>
                    <span class="save-time flex items-center gap-1">
                      <Clock :size="10" />
                      {{ formatDate(save.update_date) }}
                    </span>
                  </div>

                  <!-- Line 2: Title (Editable on Double Click) -->
                  <div class="save-title-row mt-1.5 min-h-[26px] flex items-center">
                    <input
                      v-if="editingSaveId === save.id"
                      v-model="editTitleText"
                      v-focus
                      @blur="handleSaveTitle(save.id)"
                      @keyup.enter="handleSaveTitle(save.id)"
                      class="save-title-input"
                    />
                    <div
                      v-else
                      @dblclick="startEditTitle(save)"
                      class="save-title-text select-none cursor-pointer text-white font-bold hover:text-sky-300 transition-colors duration-200 truncate"
                      title="双击以修改存档标题"
                    >
                      {{ save.title || '未命名存档' }}
                    </div>
                  </div>

                  <!-- Separator -->
                  <div class="save-separator"></div>

                  <!-- Line 3: Last Message -->
                  <div class="save-last-message mt-0.5" :title="save.last_message">
                    {{ save.last_message || '暂无对话台词记录' }}
                  </div>
                </div>
              </div>

              <!-- Bottom: Buttons -->
              <div class="save-card-bottom mt-4 flex gap-2 pt-3 border-t border-white/5">
                <button
                  @click="handleLoadSave(save.id)"
                  class="save-btn save-btn-load flex-1"
                  :disabled="actionLoading !== null"
                >
                  {{ actionLoading === save.id ? '读取中...' : '读取存档' }}
                </button>
                <button
                  @click="handleSaveGame(save.id)"
                  class="save-btn save-btn-save flex-1"
                  :disabled="actionLoading !== null"
                >
                  {{ actionLoading === save.id ? '保存中...' : '覆盖存档' }}
                </button>
                <button
                  @click="handleDeleteSave(save.id)"
                  class="save-btn save-btn-delete flex-1"
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
h3 {
  color: #eee;
  border-bottom: 1px solid #444;
  padding-bottom: 0.5rem;
  margin-bottom: 1rem;
}

.action-btn-create.glass-effect {
  background: rgba(0, 255, 55, 0.2);
  border: 1px solid rgba(0, 255, 55, 0.3);
  width: 10%;
  min-width: 65px;
  transition: all 0.2s ease;
}

.action-btn-create.glass-effect:hover {
  transform: translateY(-1px);
  background: rgba(0, 255, 55, 0.35);
  box-shadow: 0 0 10px rgba(0, 255, 55, 0.15);
}

button {
  padding: 8px 16px;
  color: #ddd;
  cursor: pointer;
  border-radius: 6px;
  transition: all 0.2s ease;
  white-space: nowrap;
}

input[type='text'] {
  width: 100%;
  padding: 10px 12px;
  border-radius: 8px;
  font-size: 15px;
  font-family: inherit;
  transition: all 0.2s ease;
  resize: vertical;
  color: #fff;
  background: rgba(255, 255, 255, 0.05);
  backdrop-filter: blur(20px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.05);
}

input[type='text']:focus {
  outline: none;
  border-color: var(--accent-color);
  box-shadow: 0 0 0 3px rgba(121, 217, 255, 0.2);
}

/* 新建存档表单 */
.new-save-form {
  display: flex;
  gap: 10px;
}

/* 存档列表 */
.save-list-container {
  max-height: 520px;
  overflow-y: auto;
  padding-right: 4px;
}

.save-list {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 20px;
  padding: 5px;
}

.status-message {
  text-align: center;
  color: #888;
  padding: 2rem;
}

.status-message.error {
  color: #ff6b6b;
}

/* 现代卡片样式 */
.save-card-modern {
  background: rgba(20, 20, 20, 0.45);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
  backdrop-filter: blur(10px);
}

.save-card-modern:hover {
  transform: translateY(-3px);
  border-color: rgba(121, 217, 255, 0.35);
  box-shadow: 0 12px 40px rgba(121, 217, 255, 0.08);
  background: rgba(20, 20, 20, 0.55);
}

.save-screenshot-container {
  width: 50%;
  height: 100%;
  border-radius: 8px;
  overflow: hidden;
  background: rgba(0, 0, 0, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.save-screenshot-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.save-screenshot-placeholder {
  width: 100%;
  height: 100%;
  background: rgba(255, 255, 255, 0.02);
}

.save-title-input {
  background: rgba(0, 0, 0, 0.5);
  border: 1px solid rgba(121, 217, 255, 0.5);
  color: #fff;
  font-size: 14px;
  font-weight: bold;
  border-radius: 4px;
  padding: 2px 6px;
  width: 100%;
  outline: none;
}

.save-title-text {
  font-size: 14px;
  font-weight: 700;
  max-width: 100%;
}

.save-separator {
  border-bottom: 1px dashed rgba(255, 255, 255, 0.15);
  margin: 8px 0;
  width: 100%;
}

.save-last-message {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.65);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.4;
  height: 33px; /* 固定高度确保一致性 */
  font-style: italic;
}

.save-btn {
  padding: 6px 12px;
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  border: none;
  color: #fff;
  white-space: nowrap;
}

.save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.save-btn-load {
  background: rgba(59, 130, 246, 0.25);
  border: 1px solid rgba(59, 130, 246, 0.4);
}

.save-btn-load:hover:not(:disabled) {
  background: rgba(59, 130, 246, 0.45);
  box-shadow: 0 0 10px rgba(59, 130, 246, 0.2);
}

.save-btn-save {
  background: rgba(16, 185, 129, 0.25);
  border: 1px solid rgba(16, 185, 129, 0.4);
}

.save-btn-save:hover:not(:disabled) {
  background: rgba(16, 185, 129, 0.45);
  box-shadow: 0 0 10px rgba(16, 185, 129, 0.2);
}

.save-btn-delete {
  background: rgba(239, 68, 68, 0.25);
  border: 1px solid rgba(239, 68, 68, 0.4);
}

.save-btn-delete:hover:not(:disabled) {
  background: rgba(239, 68, 68, 0.45);
  box-shadow: 0 0 10px rgba(239, 68, 68, 0.2);
}

/* 渐入动画 */
.animate-fade-in {
  animation: fadeIn 0.4s ease;
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@media (max-width: 900px) {
  .save-list {
    grid-template-columns: 1fr;
  }
}
</style>
