<template>
  <MenuPage>
    <MenuItem title="◈ 历史对话">
      <div class="history-container">
        <div v-if="dialogHistory.length === 0" class="status-message">
          暂无历史记录，去和ta聊聊天叭(*^▽^*)
        </div>
        <div v-else class="chat-history-container" ref="chatContainer">
          <template
            v-for="(message, index) in dialogHistory"
            :key="message.timestamp || index"
          >
            <div class="message-item">
              <div v-if="message.type === 'message'" class="player-message">
                <span class="name">{{ message.character }}: </span>
                <span class="content">{{ message.content }}</span>
              </div>

              <div
                v-else-if="message.type === 'reply'"
                class="ai-reply"
                :title="`情绪: ${message.originalTag}`"
              >
                <span class="name">{{ message.character }}: </span>
                <span class="content">
                  {{ message.content }}
                  <template v-if="message.motionText"
                    >({{ message.motionText }})</template
                  >
                </span>
              </div>
            </div>

            <div v-if="message.isFinal" class="final-spacer"></div>
          </template>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
// 1. 从 vue 中引入 ref 和 watch
import { computed, ref, watch } from "vue";
import { MenuPage, MenuItem } from "../../ui";

import { useGameStore } from "../../../stores/modules/game";
import type { DialogMessage } from "../../../stores/modules/game/state";

const gameStore = useGameStore();

const dialogHistory = computed<DialogMessage[]>(() => gameStore.dialogHistory);

// 1. 创建一个 ref 来关联模板中的聊天容器元素
const chatContainer = ref<HTMLElement | null>(null);

// 1. 侦听 dialogHistory 的变化
watch(
  dialogHistory,
  () => {
    // Vue 会在DOM更新后调用这个回调函数
    // 确保 chatContainer.value 存在
    if (chatContainer.value) {
      // 将容器的滚动位置设置到最底部
      chatContainer.value.scrollTop = chatContainer.value.scrollHeight;
    }
  },
  {
    // flush: 'post' 确保回调在Vue更新DOM之后执行，这样我们才能获取到正确的 scrollHeight
    flush: "post",
    // deep: true 确保能侦听到数组内部对象的变化（虽然对于数组push等操作不是必须的，但是个好习惯）
    deep: true,
  }
);
</script>

<style scoped>
.history-container {
  height: 500px; /* 如果内容过多，可以设置最大高度和滚动条 */
}

.chat-history-container {
  padding: 10px;
  display: flex;
  flex-direction: column; /* 让消息垂直排列 */
  gap: 12px; /* 消息之间的间距 */
  font-size: 18px;
  max-height: 500px; /* 如果内容过多，可以设置最大高度和滚动条 */
  overflow-y: auto;
  scroll-behavior: smooth;
}

.message-item {
  line-height: 1.2;
  word-wrap: break-word;
}

.name {
  font-weight: bold;
}

.ai-reply {
  cursor: help;
  display: inline-block; /* 让边框包裹内容 */
}

/* 为 isFinal 消息的间隔元素添加样式 */
.final-spacer {
  height: 1em; /* 高度约等于一个空行 */
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
