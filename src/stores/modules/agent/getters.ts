/**
 * 剧本编辑器 AI 助手 store —— 计算属性。
 */
import { computed } from 'vue'
import { useAgentState } from './state'

export function useAgentGetters(state: ReturnType<typeof useAgentState>) {
  const hasContent = computed(() => state.items.value.length > 0)

  const currentConversation = computed(
    () =>
      state.conversations.value.find((c) => c.id === state.currentId.value) ?? null,
  )

  return { hasContent, currentConversation }
}

export type AgentGetters = ReturnType<typeof useAgentGetters>
