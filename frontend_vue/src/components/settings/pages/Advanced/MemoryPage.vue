<template>
  <section v-if="view === 'memories'" class="space-y-4">
    <div
      v-if="showEditor"
      class="rounded-lg border border-cyan-300/25 bg-slate-900/25 p-4 space-y-3"
    >
      <textarea
        v-model="form.content"
        class="w-full min-h-28 rounded-lg bg-black/25 border border-white/10 p-3 text-sm text-white outline-none focus:border-cyan-300 resize-y"
        placeholder="写下希望 LingChat AI 记住的内容..."
      ></textarea>
      <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
        <input
          v-model="form.tagsText"
          class="rounded-lg bg-black/25 border border-white/10 px-3 py-2 text-white outline-none focus:border-cyan-300"
          placeholder="标签，用逗号分隔"
        />
        <input
          v-model="form.source"
          class="rounded-lg bg-black/25 border border-white/10 px-3 py-2 text-white outline-none focus:border-cyan-300"
          placeholder="来源，例如 主人 / AI / Codex"
        />
      </div>
      <div class="flex justify-end gap-2">
        <button class="action-btn" @click="showEditor = false">取消</button>
        <button
          class="action-btn bg-cyan-500/35 hover:bg-cyan-500/50"
          :disabled="!form.content.trim()"
          @click="saveMemory"
        >
          <Save :size="16" />
          保存记忆
        </button>
      </div>
    </div>

    <div
      v-if="memories.length === 0"
      class="min-h-56 rounded-lg border border-dashed border-cyan-300/25 bg-slate-900/20 flex flex-col items-center justify-center text-white/55"
    >
      <Brain :size="38" class="mb-3 text-cyan-200/80" />
      <p>还没有记忆</p>
      <p class="text-xs mt-2">可以手动添加，也可以让 AI 用工具写入这里</p>
    </div>

    <div v-else class="space-y-3">
      <article
        v-for="memory in memories"
        :key="memory.id"
        class="rounded-lg border border-cyan-300/20 bg-slate-900/25 p-4 text-white"
      >
        <div class="flex items-start justify-between gap-3">
          <p class="text-sm leading-relaxed whitespace-pre-wrap">{{ memory.content }}</p>
          <button
            class="p-1 rounded hover:bg-rose-500/25 text-white/65 hover:text-rose-100 shrink-0"
            title="删除"
            @click="removeMemory(memory.id)"
          >
            <Trash2 :size="16" />
          </button>
        </div>
        <div class="mt-3 flex flex-wrap items-center gap-2 text-xs text-white/55">
          <span v-if="memory.source" class="rounded bg-white/10 px-2 py-1">{{
            memory.source
          }}</span>
          <span v-if="memory.createdAt">{{ formatTime(memory.createdAt) }}</span>
          <span
            v-for="tag in memory.tags || []"
            :key="memory.id + '-' + tag"
            class="rounded bg-cyan-400/15 text-cyan-100 px-2 py-1"
          >
            #{{ tag }}
          </span>
        </div>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { Brain, Save, Trash2 } from 'lucide-vue-next'
import { getAdvancedFeatures, saveAdvancedFeatures } from '@/api/services/advanced'

interface MemoryNote {
  id: string
  content: string
  tags?: string[]
  source?: string
  createdAt?: string
}

const props = defineProps<{
  view: string
}>()

const memories = ref<MemoryNote[]>([])
const showEditor = ref(false)
const form = reactive({
  content: '',
  tagsText: '',
  source: '主人',
})

const loadData = async () => {
  try {
    const data = await getAdvancedFeatures()
    memories.value = data.memoryNotes || []
  } catch (error) {
    console.error('Failed to load memory notes', error)
  }
}

const persist = async () => {
  await saveAdvancedFeatures({ memoryNotes: memories.value })
}

const handleCreate = () => {
  form.content = ''
  form.tagsText = ''
  form.source = '主人'
  showEditor.value = true
}

let memoryIdCounter = 0

const saveMemory = async () => {
  const content = form.content.trim()
  if (!content) return

  const tags = form.tagsText
    .split(/[,，]/)
    .map((tag) => tag.trim())
    .filter(Boolean)

  memories.value.unshift({
    id: `mem_${Date.now()}_${++memoryIdCounter}`,
    content,
    tags,
    source: form.source.trim() || '主人',
    createdAt: new Date().toISOString(),
  })

  await persist()
  showEditor.value = false
}

const removeMemory = async (id: string) => {
  memories.value = memories.value.filter((item) => item.id !== id)
  await persist()
}

const formatTime = (value?: string) => {
  if (!value) return ''
  return new Date(value).toLocaleString()
}

let isMounted = false

const handleDataUpdated = () => {
  if (isMounted) loadData()
}

onMounted(() => {
  isMounted = true
  loadData()
  window.addEventListener('advanced-features-updated', handleDataUpdated)
})

onBeforeUnmount(() => {
  isMounted = false
  window.removeEventListener('advanced-features-updated', handleDataUpdated)
})

defineExpose({ handleCreate })
</script>

<style scoped>
.action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  border-radius: 0.5rem;
  background: rgba(255, 255, 255, 0.12);
  color: white;
  padding: 0.5rem 0.75rem;
  font-size: 0.875rem;
  transition: background 0.2s ease;
}

.action-btn:hover {
  background: rgba(255, 255, 255, 0.2);
}

.action-btn:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}
</style>
