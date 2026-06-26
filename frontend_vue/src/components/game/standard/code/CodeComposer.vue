<template>
  <div class="code-composer-area">
    <!-- 快捷操作按钮 -->
    <div class="quick-actions">
      <button type="button" @click="quickSend('继续执行当前计划')">
        <Sparkles :size="14" />
        继续
      </button>
      <button type="button" @click="quickSend('生成一个简短计划')">
        <ListChecks :size="14" />
        计划
      </button>
      <button type="button" @click="quickSend('总结以上你的所有操作')">
        <ClipboardList :size="14" />
        总结
      </button>
    </div>

    <!-- 输入框 -->
    <form class="code-composer" @submit.prevent="handleSubmit">
      <textarea
        ref="inputRef"
        v-model="inputMessage"
        rows="2"
        class="code-input"
        :placeholder="placeholderText"
        :readonly="!canSend"
        @keydown.enter.exact.prevent="handleSubmit"
      ></textarea>
      <div class="composer-footer">
        <div class="composer-tools">
          <button
            type="button"
            :title="isTouchMode ? '退出触摸模式' : '触摸模式'"
            :class="{ 'tool-enabled': isTouchMode }"
            @click="$emit('toggle-touch')"
          >
            <Hand :size="15" />
          </button>
          <button type="button" title="Code TTS 设置" @click="$emit('open-settings')">
            <Volume2 :size="15" />
          </button>
        </div>
        <button class="send-button" type="submit" :disabled="!canSubmit" title="发送">
          <SendHorizontal :size="18" />
        </button>
      </div>
    </form>

    <div class="model-line">{{ modelLabel }} · {{ statusText }}</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import {
  ClipboardList,
  Hand,
  ListChecks,
  SendHorizontal,
  Sparkles,
  Volume2,
} from 'lucide-vue-next'

const props = defineProps<{
  modelLabel: string
  statusText: string
  canSend: boolean
  isTouchMode: boolean
  placeholderText?: string
}>()

const emit = defineEmits<{
  send: [text: string]
  'open-settings': []
  'toggle-touch': []
}>()

const inputMessage = ref('')
const inputRef = ref<HTMLTextAreaElement | null>(null)

// 内部判断能否提交
const canSubmit = computed(() => props.canSend && inputMessage.value.trim().length > 0)

// 统一的发送入口（表单提交 / Enter / 快捷发送共用）
const sendText = (text: string) => {
  if (!props.canSend || !text.trim()) return
  inputMessage.value = ''
  emit('send', text.trim())
}

const handleSubmit = () => sendText(inputMessage.value)

const quickSend = (text: string) => sendText(text)

defineExpose({ inputRef })
</script>

<style scoped>
.code-composer-area {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.quick-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.45rem;
  padding: 0.2rem 0.95rem 0.65rem;
}

.quick-actions button {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  border: 1px solid rgba(125, 211, 252, 0.2);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.1);
  padding: 0.45rem 0.62rem;
  color: rgb(224, 242, 254);
  font-size: 0.75rem;
  cursor: pointer;
  transition: background 0.2s;
}

.quick-actions button:hover {
  background: rgba(125, 211, 252, 0.18);
}

.code-composer {
  margin: 0 0.95rem 0.95rem;
  border: 1px solid rgba(125, 211, 252, 0.45);
  border-radius: 16px;
  background: rgba(15, 23, 42, 0.65);
  box-shadow:
    0 12px 34px rgba(0, 0, 0, 0.35),
    inset 0 1px 1px rgba(255, 255, 255, 0.15);
}

.code-input {
  display: block;
  width: 100%;
  min-height: 4rem;
  max-height: 9rem;
  resize: none;
  border: 0;
  background: transparent;
  padding: 0.75rem 0.8rem 0.35rem;
  color: rgb(248, 250, 252);
  font-size: 0.875rem;
  line-height: 1.5;
  outline: none;
  text-shadow: none;
}

.code-input::placeholder {
  color: rgba(203, 213, 225, 0.52);
}

.composer-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.35rem 0.55rem 0.55rem;
}

.composer-tools {
  display: flex;
  align-items: center;
  gap: 0.35rem;
}

.composer-tools button {
  display: grid;
  width: 1.75rem;
  height: 1.75rem;
  place-items: center;
  border-radius: 9px;
  border: none;
  background: none;
  color: rgba(224, 242, 254, 0.72);
  cursor: pointer;
  transition: background 0.2s, color 0.2s;
}

.composer-tools button:hover {
  background: rgba(255, 255, 255, 0.12);
  color: white;
}

.composer-tools button.tool-enabled {
  background: rgba(125, 211, 252, 0.2);
  color: rgb(186, 230, 253);
  box-shadow: inset 0 0 0 1px rgba(125, 211, 252, 0.28);
}

.send-button {
  display: grid;
  width: 2.2rem;
  height: 2.2rem;
  place-items: center;
  border: none;
  border-radius: 50%;
  background: rgb(59, 130, 246);
  color: white;
  box-shadow: 0 0 22px rgba(59, 130, 246, 0.35);
  cursor: pointer;
  transition: background 0.2s;
}

.send-button:hover:not(:disabled) {
  background: rgb(14, 165, 233);
}

.send-button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.model-line {
  padding: 0 0.72rem 0.58rem;
  color: rgba(203, 213, 225, 0.62);
  font-size: 0.6875rem;
}
</style>
