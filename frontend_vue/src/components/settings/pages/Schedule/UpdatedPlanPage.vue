<template>
  <section v-if="uiStore.scheduleView === 'todo_groups'" class="space-y-4">
    <div class="flex items-center justify-between gap-3">
      <div>
        <h3 class="text-xs font-black text-slate-50 uppercase tracking-[0.2em] flex items-center">
          <ListChecks class="w-3 h-3 mr-2 text-cyan-300" />
          AI 当前计划
        </h3>
        <p class="mt-1 text-xs text-white/55">这里显示 AI 当前任务进度，和下面的长期待办分开保存</p>
      </div>
      <button class="action-btn bg-cyan-500/25 hover:bg-cyan-500/40 shrink-0" @click="handleCreate">
        <Save :size="16" />
        {{ plan ? '编辑计划' : '新建计划' }}
      </button>
    </div>

    <div
      v-if="showEditor"
      class="rounded-lg border border-cyan-300/25 bg-slate-900/25 p-4 space-y-3"
    >
      <input
        v-model="form.title"
        class="w-full rounded-lg bg-black/25 border border-white/10 px-3 py-2 text-white outline-none focus:border-cyan-300"
        placeholder="计划标题"
      />
      <textarea
        v-model="form.lines"
        class="w-full min-h-40 rounded-lg bg-black/25 border border-white/10 p-3 text-sm text-white outline-none focus:border-cyan-300 resize-y"
        placeholder="待处理：确认需求&#10;进行中：实现工具&#10;已完成：构建验证"
      ></textarea>
      <div class="flex justify-end gap-2">
        <button class="action-btn" @click="showEditor = false">取消</button>
        <button
          class="action-btn bg-cyan-500/35 hover:bg-cyan-500/50"
          :disabled="!canSave"
          @click="savePlan"
        >
          <Save :size="16" />
          保存计划
        </button>
      </div>
    </div>

    <div
      v-if="!plan"
      class="min-h-56 rounded-lg border border-dashed border-cyan-300/25 bg-slate-900/20 flex flex-col items-center justify-center text-white/55"
    >
      <ListChecks :size="38" class="mb-3 text-cyan-200/80" />
      <p>还没有 AI 当前计划</p>
      <p class="text-xs mt-2">AI 可以用 update_plan 工具把进度写到这里</p>
    </div>

    <article v-else class="rounded-lg border border-cyan-300/20 bg-slate-900/25 p-4 text-white">
      <div class="flex items-start justify-between gap-3">
        <div>
          <h3 class="text-lg font-bold text-cyan-50">{{ plan.title || 'Updated Plan' }}</h3>
          <div class="mt-2 flex flex-wrap items-center gap-2 text-xs text-white/55">
            <span v-if="plan.source" class="rounded bg-white/10 px-2 py-1">{{ plan.source }}</span>
            <span v-if="plan.updatedAt">{{ formatTime(plan.updatedAt) }}</span>
          </div>
        </div>
        <button
          class="p-1 rounded hover:bg-rose-500/25 text-white/65 hover:text-rose-100 shrink-0"
          title="清空"
          @click="clearPlan"
        >
          <Trash2 :size="16" />
        </button>
      </div>

      <ol class="mt-4 space-y-2">
        <li
          v-for="(item, index) in plan.items || []"
          :key="index + '-' + item.step"
          class="flex items-start gap-3 rounded-lg bg-black/20 border border-white/10 px-3 py-2"
        >
          <span
            :class="['mt-1 h-2.5 w-2.5 rounded-full shrink-0', statusDotClass(item.status)]"
          ></span>
          <div class="min-w-0">
            <p
              :class="[
                'text-sm leading-relaxed',
                item.status === 'completed' ? 'line-through text-white/55' : 'text-white',
              ]"
            >
              {{ item.step }}
            </p>
            <p v-if="item.note" class="mt-1 text-xs text-white/50">{{ item.note }}</p>
          </div>
        </li>
      </ol>
    </article>
  </section>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { ListChecks, Save, Trash2 } from 'lucide-vue-next'
import { getSchedules, saveSchedules } from '@/api/services/schedule'
import { useUIStore } from '@/stores/modules/ui/ui'

type PlanStatus = 'pending' | 'in_progress' | 'completed' | 'cancelled'

interface PlanItem {
  step: string
  status: PlanStatus
  note?: string
}

interface UpdatedPlan {
  title?: string
  items?: PlanItem[]
  source?: string
  updatedAt?: string
}

const uiStore = useUIStore()
const plan = ref<UpdatedPlan | null>(null)
const showEditor = ref(false)
const form = reactive({
  title: 'Updated Plan',
  lines: '',
})

const canSave = computed(() => parseLines(form.lines).length > 0)

const loadData = async () => {
  try {
    const data = await getSchedules()
    plan.value = data.updatedPlan || null
  } catch (error) {
    console.error('Failed to load updated plan', error)
  }
}

const persist = async (value: UpdatedPlan | null) => {
  await saveSchedules({ updatedPlan: value })
}

const handleCreate = () => {
  form.title = plan.value?.title || 'Updated Plan'
  form.lines = (plan.value?.items || [])
    .map(
      (item) => `${statusLabel(item.status)}：${item.step}${item.note ? ` ｜ ${item.note}` : ''}`,
    )
    .join('\n')
  showEditor.value = true
}

const savePlan = async () => {
  const items = parseLines(form.lines)
  if (items.length === 0) return

  plan.value = {
    title: form.title.trim() || 'Updated Plan',
    items,
    source: '主人',
    updatedAt: new Date().toISOString(),
  }
  await persist(plan.value)
  showEditor.value = false
}

const clearPlan = async () => {
  plan.value = null
  await persist(null)
}

const parseLines = (value: string): PlanItem[] => {
  return value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const parts = line.split(/[:：]/, 2)
      const statusText = parts[0] || ''
      const rest = parts[1] || ''
      const detailParts = (rest || statusText).split(/｜|\|/, 2)
      const stepText = detailParts[0] || ''
      const noteText = detailParts[1] || ''
      return {
        step: stepText.trim(),
        status: parseStatus(rest ? statusText.trim() : ''),
        note: noteText.trim() || undefined,
      }
    })
    .filter((item) => item.step)
}

const parseStatus = (value: string): PlanStatus => {
  if (['进行中', 'in_progress', 'doing'].includes(value)) return 'in_progress'
  if (['已完成', '完成', 'completed', 'done'].includes(value)) return 'completed'
  if (['取消', '已取消', 'cancelled', 'canceled'].includes(value)) return 'cancelled'
  return 'pending'
}

const statusLabel = (status?: string) => {
  if (status === 'in_progress') return '进行中'
  if (status === 'completed') return '已完成'
  if (status === 'cancelled') return '已取消'
  return '待处理'
}

const statusDotClass = (status?: string) => {
  if (status === 'in_progress') return 'bg-cyan-300 shadow-[0_0_10px_rgba(103,232,249,0.75)]'
  if (status === 'completed') return 'bg-emerald-400'
  if (status === 'cancelled') return 'bg-rose-400'
  return 'bg-white/35'
}

const formatTime = (value?: string) => {
  if (!value) return ''
  return new Date(value).toLocaleString()
}

const handleScheduleUpdated = () => {
  loadData()
}

onMounted(() => {
  loadData()
  window.addEventListener('schedule-updated', handleScheduleUpdated)
})

onBeforeUnmount(() => {
  window.removeEventListener('schedule-updated', handleScheduleUpdated)
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
