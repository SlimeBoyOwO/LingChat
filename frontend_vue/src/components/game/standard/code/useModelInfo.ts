/**
 * 模型信息 Composable
 * 加载并缓存当前 LLM 模型名称/提供商
 */
import { ref, onMounted, watch } from 'vue'
import http from '@/api/http'
import { useSettingsStore } from '@/stores/modules/settings'

const lastKnownModel = ref(localStorage.getItem('lingchat_last_model') || '')

export function useModelInfo() {
  const currentModelLabel = ref(lastKnownModel.value)

  async function loadCurrentModel() {
    try {
      const res = await http.get('/v1/chat/info/llm_model', { silent: true })
      const data = res?.data || res || {}
      const provider = data.provider || 'unknown'
      const model = data.model || 'unknown'
      const label = `${provider} / ${model}`
      currentModelLabel.value = label
      lastKnownModel.value = label
      localStorage.setItem('lingchat_last_model', label)
    } catch {
      currentModelLabel.value = lastKnownModel.value || '未知模型'
    }
  }

  onMounted(loadCurrentModel)

  // LLM 配置变更时自动重新加载模型信息
  watch(
    () => useSettingsStore().text.codeMode,
    () => {
      // codeMode 切换后延后加载，给后端足够时间完成配置切换
      setTimeout(loadCurrentModel, 500)
    },
  )

  return { currentModelLabel }
}
