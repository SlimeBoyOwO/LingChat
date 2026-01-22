<template>
  <MenuPage>
    <MenuItem title="创建新存档">
      <div class="new-save-form">
        <Input
          type="text"
          v-model="newSaveTitle"
          placeholder="输入存档名称"
          @keyup.enter="handleCreateSave"
        />
        <button class="glass-effect action-btn-create" @click="handleCreateSave">创建</button>
      </div>
    </MenuItem>
    <MenuItem title="存档列表">
      <div class="save-section">
        <div class="save-list-container">
          <div v-if="loading" class="status-message">加载中...</div>

          <div v-else-if="error" class="status-message error">加载失败: {{ error }}</div>

          <div v-else-if="saves.length === 0" class="status-message">暂无存档记录</div>

          <div v-else class="save-list">
            <div v-for="save in saves" :key="save.id" class="save-card glass-effect">
              <div class="save-info">
                <span class="save-title">{{ save.title || '未命名存档' }}</span>
                <span class="save-date">{{ formatDate(save.update_date) }}</span>
              </div>
              <div class="save-actions">
                <button @click="handleLoadSave(save.id)" class="glass-effect action-btn-load">
                  读取
                </button>
                <button @click="handleSaveGame(save.id)" class="action-btn-save glass-effect">
                  保存
                </button>
                <button @click="handleDeleteSave(save.id)" class="action-btn-delete glass-effect">
                  删除
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
import { MenuPage, MenuItem } from '../../ui' // 假设的UI组件路径
import { Input, Button } from '../../base'
import { useGameStore } from '../../../stores/modules/game'
import {
  saveGetAll,
  saveCreate,
  saveLoad,
  saveGameSave,
  saveDelete,
} from '../../../api/services/save'
import type { SaveInfo } from '../../../types'
import { useUserStore } from '../../../stores/modules/user/user'

// 定义存档对象类型

// 使用 Pinia Store
const gameStore = useGameStore()
const userStore = useUserStore()

// 组件响应式状态
const saves = ref<SaveInfo[]>([])
const newSaveTitle = ref('')
const loading = ref(false)
const error = ref<string | null>(null)

// 模拟用户ID，实际应用中应从认证状态中获取
const userId = '1'

/**
 * 格式化日期
 * @param dateString ISO格式的日期字符串
 */
const formatDate = (dateString: string): string => {
  const date = new Date(dateString)
  return `${date.getFullYear()}.${date.getMonth() + 1}.${date.getDate()}`
}

/**
 * 1. 从后端获取存档列表
 */
const fetchSaves = async () => {
  loading.value = true
  error.value = null
  try {
    // 假设 request.historyList 返回存档数组
    const saveListData = await saveGetAll({
      user_id: userId,
      page: 1,
      page_size: 10,
    })
    console.log('获取存档列表成功:', saveListData)
    saves.value = saveListData.saves
  } catch (e: any) {
    console.error('获取存档列表失败:', e)
    error.value = e.message || '未知错误'
  } finally {
    loading.value = false
  }
}

/**
 * 2. 创建一个新的存档
 */
const handleCreateSave = async () => {
  if (!newSaveTitle.value.trim()) {
    alert('请输入存档名称！')
    return
  }
  try {
    // 调用创建接口
    await saveCreate({ user_id: userId, title: newSaveTitle.value.trim() })
    newSaveTitle.value = '' // 清空输入框
    await fetchSaves() // 重新加载列表
  } catch (e: any) {
    console.error('创建存档失败:', e)
    alert(`创建失败: ${e.message}`)
  }
}

/**
 * 3. 读取存档
 * @param saveId 存档ID
 */
const handleLoadSave = async (saveId: string) => {
  try {
    const saveData = await saveLoad({
      user_id: userId,
      conversation_id: saveId,
    })
    // gameStore.loadDialogHistory(saveData);  TODO: 读取的时候，把历史记录也加载进去
    alert(`存档 [${saveId}] 读取成功!`)
    gameStore.initializeGame(userStore.client_id, userId)
  } catch (e: any) {
    console.error('读取存档失败:', e)
    alert(`读取失败: ${e.message}`)
  }
}

/**
 * 4. 保存游戏到指定存档位
 * @param saveId 存档ID
 */
const handleSaveGame = async (saveId: string) => {
  try {
    await saveGameSave({
      user_id: userId,
      conversation_id: saveId,
    })
    alert(`成功覆盖存档 [${saveId}]!`)
    await fetchSaves() // 刷新列表以更新时间戳
  } catch (e: any) {
    console.error('保存游戏失败:', e)
    alert(`保存失败: ${e.message}`)
  }
}

/**
 * 5. 删除存档
 * @param saveId 存档ID
 */
const handleDeleteSave = async (saveId: string) => {
  if (confirm('确定要删除这个存档吗？此操作不可撤销。')) {
    try {
      await saveDelete({
        user_id: userId,
        conversation_id: saveId,
      })
      await fetchSaves() // 刷新列表
    } catch (e: any) {
      console.error('删除存档失败:', e)
      alert(`删除失败: ${e.message}`)
    }
  }
}

// 组件挂载后，自动加载存档列表
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
