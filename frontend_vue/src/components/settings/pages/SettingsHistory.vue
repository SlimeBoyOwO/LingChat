<template>
  <MenuPage>
    <MenuItem title="◈ 历史对话">
      <div class="history-container">
        <div v-if="dialogHistory.length === 0" class="status-message">
          暂无历史记录，去和ta聊聊天叭(*^▽^*)
        </div>
        <div v-else class="chat-history-container" ref="chatContainer">
          <DialogSession :dialog="dialogHistory" />
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
// 1. 从 vue 中引入 ref 和 watch
import { computed } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { useGameStore } from '../../../stores/modules/game'
import type { GameMessage } from '../../../stores/modules/game/state'
import DialogSession from '../history/DialogSession.vue'

const gameStore = useGameStore()

const dialogHistory = computed<GameMessage[]>(() => gameStore.dialogHistory)
</script>

<style scoped>
.history-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.chat-history-container {
  padding: 10px;
  display: flex;
  flex-direction: column; /* 让消息垂直排列 */
  gap: 12px; /* 消息之间的间距 */
  font-size: 18px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scroll-behavior: smooth;
}

.status-message {
  text-align: center;
  color: #f5f5f5;
  padding: 2rem;
  font-size: 24px;
  font-weight: bold;
  height: 100%;
  text-shadow: 0 0 5px rgba(255, 255, 255, 0.5);
}
</style>
