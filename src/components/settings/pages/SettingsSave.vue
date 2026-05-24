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
            <div
              v-for="save in saves"
              :key="save.id"
              class="relative flex rounded-xl p-8 border justify-between transition-all duration-300 group bg-black/30 border-white/20 hover:bg-black/25 hover:border-black/5 shadow-lg hover:shadow-emerald-500/10 hover:-translate-y-0.5"
            >
              <div
                class="absolute left-0 top-1/2 -translate-y-1/2 w-1 h-0 bg-brand transition-all duration-300 group-hover:h-1/2 rounded-r-md"
              ></div>
              <div class="flex rounded-xl items-center justify-center text-brand gap-8">
                <Save></Save>
                <div class="flex flex-col gap-1">
                  <div class="text-base font-bold text-white">{{ save.title || '未命名存档' }}</div>
                  <div class="flex items-baseline gap-2">
                    <span><Clock :size="14" /></span>
                    <span class="save-date">{{ formatDate(save.update_date) }}</span>
                  </div>
                </div>
              </div>

              <div class="save-actions">
                <button
                  @click="handleLoadSave(save.id)"
                  class="glass-effect action-btn-load"
                  :disabled="actionLoading !== null"
                >
                  {{ actionLoading === save.id ? '加载中...' : '读取' }}
                </button>
                <button
                  @click="handleSaveGame(save.id)"
                  class="action-btn-save glass-effect"
                  :disabled="actionLoading !== null"
                >
                  {{ actionLoading === save.id ? '保存中...' : '保存' }}
                </button>
                <button
                  @click="handleDeleteSave(save.id)"
                  class="action-btn-delete glass-effect"
                  :disabled="actionLoading !== null"
                >
                  {{ actionLoading === save.id ? '删除中...' : '删除' }}
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
import { invoke } from '@tauri-apps/api/core'
import type { SaveInfo } from '../../../types'
import type { WebInitData } from '../../../api/services/game-info'
import { Save, PencilLine, LayoutList, Clock } from 'lucide-vue-next'

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

const formatDate = (dateString: string): string => {
  const date = new Date(dateString)
  return `${date.getFullYear()}.${date.getMonth() + 1}.${date.getDate()}`
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

const handleCreateSave = async () => {
  if (!newSaveTitle.value.trim()) {
    uiStore.showWarning({ title: '提示', message: '请输入存档名称' })
    return
  }
  actionLoading.value = -1
  try {
    await invoke<CreateSaveResponse>('create_save', {
      title: newSaveTitle.value.trim(),
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
  const confirmed = await dialogStore.confirm(
    '覆盖存档会导致丢失之前的存档进度，确定要覆盖吗？',
  )
  if (!confirmed) return
  actionLoading.value = saveId
  try {
    await invoke('update_save', { saveId })
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
  if (!await dialogStore.confirm('确定要删除这个存档吗？此操作不可撤销。')) return
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
/* 通用样式 */
/*.save-section {
  
}
*/

h3 {
  color: #eee;
  border-bottom: 1px solid #444;
  padding-bottom: 0.5rem;
  margin-bottom: 1rem;
}

.action-btn-create.glass-effect {
  background: rgba(0, 255, 55, 0.3);
  width: 10%;
  min-width: 60px;
  transition: all 0.2s ease;
}

.action-btn-load.glass-effect {
  background: rgba(0, 123, 255, 0.3);
  transition: all 0.2s ease;
}

.action-btn-save.glass-effect {
  background: rgba(0, 255, 43, 0.3);
  transition: all 0.2s ease;
}

.action-btn-delete.glass-effect {
  background: rgba(255, 0, 0, 0.3);
  transition: all 0.2s ease;
}

.action-btn-create.glass-effect:hover {
  transform: translateY(-1px);
  background: rgba(0, 194, 42, 0.3);
}

.action-btn-load.glass-effect:hover {
  transform: translateY(-1px);
  background: rgba(0, 96, 199, 0.3);
}

.action-btn-save.glass-effect:hover {
  transform: translateY(-1px);
  background: rgba(0, 199, 33, 0.3);
}

.action-btn-delete.glass-effect:hover {
  transform: translateY(-1px);
  background: rgba(207, 0, 0, 0.3);
}

button {
  padding: 8px 16px;
  border: 0px solid #555;
  color: #ddd;
  cursor: pointer;
  border-radius: 4px;
  transition:
    background-color 0.2s,
    border-color 0.2s;
  white-space: nowrap;
}

input[type='text'] {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid #fff;
  border-radius: 8px;
  font-size: 15px;
  font-family: inherit;
  transition:
    border-color 0.2s,
    box-shadow 0.2s;
  resize: vertical;

  color: #fff;
  background: rgba(255, 255, 255, 0.1);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(255, 255, 255, 0.125);
  box-shadow:
    0 8px 32px rgba(0, 0, 0, 0.1),
    inset 0 1px 1px rgba(255, 255, 255, 0.1);
}

input[type='text'] :focus {
  outline: none;
  border-color: var(--accent-color);
  box-shadow: 0 0 0 3px rgba(0, 123, 255, 0.2);
}

/* 新建存档表单 */
.new-save-form {
  display: flex;
  gap: 10px;
}

/* 存档列表 */
.save-list-container {
  max-height: 400px;
  overflow-y: auto;
}

.save-list {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 20px;
  padding: 10px;
}

.status-message {
  text-align: center;
  color: #888;
  padding: 2rem;
}

.status-message.error {
  color: #ff6b6b;
}

.save-card {
  display: flex;
  justify-content: space-between;
  align-items: center;
  background-color: rgba(30, 30, 30, 0.8);
  padding: 1rem;
  border-radius: 6px;
  margin-bottom: 0.75rem;
  border: 1px solid #444;
}

.save-card.glass-effect {
  transition: all 0.3s ease;
}

.save-card:hover {
  transform: translateY(-2px);
}

.save-info {
  display: flex;
  flex-direction: column;
}

.save-title {
  font-size: 1rem;
  font-weight: bold;
  color: #fff;
}

.save-date {
  font-size: 0.8rem;
  color: #eaeaea;
  margin-top: 4px;
}

.save-actions {
  display: flex;
  gap: 8px;
}

@media (max-width: 900px) {
  .save-list {
    grid-template-columns: 1fr;
  }
}
</style>
