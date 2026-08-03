/**
 * 剧本编辑器 AI 助手（Skill Agent）store。
 *
 * setup 风格（与 script-editor store 一致）。对话在 DB、设置在 settings.json
 * 后端持久化，前端只做会话内缓存，因此不启用 persist。
 */
import { defineStore } from 'pinia'
import { useAgentActions } from './actions'
import { useAgentGetters } from './getters'
import { useAgentState } from './state'

export const useAgentStore = defineStore('agent', () => {
  const s = useAgentState()
  const g = useAgentGetters(s)
  const a = useAgentActions(s)
  return { ...s, ...g, ...a }
})
