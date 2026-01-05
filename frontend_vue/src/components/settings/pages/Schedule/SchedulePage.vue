<template>
  <!-- 视图：日程主题列表 -->
  <div
    v-if="uiStore.scheduleView === 'schedule_groups'"
    class="grid grid-cols-1 sm:grid-cols-1 lg:grid-cols-2 gap-6"
  >
    <div
      v-for="(group, id) in scheduleGroups"
      :key="id"
      @click="selectGroup(id)"
      class="group glass-effect p-6 rounded-3xl border border-brand shadow-sm hover:shadow-xl hover:-translate-y-1 transition-all cursor-pointer"
    >
      <div
        class="w-12 h-12 bg-cyan-500 rounded-2xl flex items-center justify-center text-cyan-50 mb-4 group-hover:bg-cyan-50 group-hover:text-cyan-500 transition-colors"
      >
        <FolderKanban></FolderKanban>
      </div>
      <h3 class="font-bold text-lg text-brand">
        {{ group.title }}
      </h3>
      <p class="text-sm text-white mt-2 line-clamp-2">
        {{ group.description }}
      </p>
      <div
        class="mt-6 pt-4 border-t border-slate-50 flex justify-between items-center text-xs font-bold text-brand"
      >
        <span>{{ group.items.length }} 个日程</span>
        <ArrowRight :size="16" />
      </div>
    </div>
  </div>

  <!-- 视图：日程详情列表 -->
  <div v-if="uiStore.scheduleView === 'schedule_detail'" class="max-w-3xl mx-auto space-y-4">
    <div
      v-for="(item, idx) in activeGroup.items"
      :key="idx"
      class="glass-effect p-5 rounded-2xl border border-slate-100 shadow-sm flex items-start space-x-4"
    >
      <div class="bg-cyan-500 text-white px-3 py-1 rounded-lg text-xs font-bold self-center">
        {{ item.time }}
      </div>
      <div class="flex-1">
        <h4 class="font-bold text-brand text-lg">{{ item.name }}</h4>
        <p class="text-sm text-white mt-1">{{ item.content }}</p>
      </div>
      <button @click="removeScheduleItem(idx)" class="text-slate-300 hover:text-red-400 p-1">
        <Trash2 />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { ArrowRight, Trash2, FolderKanban } from 'lucide-vue-next'

const uiStore = useUIStore()

// 数据存储
interface ScheduleItem {
  name: string
  time: string
  content: string
}

interface ScheduleGroup {
  title: string
  description: string
  items: ScheduleItem[]
}

const scheduleGroups = ref<Record<string, ScheduleGroup>>({
  g1: {
    title: '莱姆的日常生活',
    description: '涵盖了每日的起床、工作、休息提醒。',
    items: [
      { name: '早起问候', time: '12:00', content: '问候莱姆起床' },
      { name: '午休结束', time: '14:00', content: '提醒写代码' },
    ],
  },
  g2: {
    title: '期末复习周',
    description: '12月考试冲刺期间的特殊时间分配。',
    items: [],
  },
})

const activeGroup = computed(() => {
  if (!selectedGroupId.value) {
    return { items: [] }
  }
  return scheduleGroups.value[selectedGroupId.value] || { items: [] }
})

const openModal = () => {}
const removeScheduleItem = (idx: number) => {
  activeGroup.value.items.splice(idx, 1)
}

const selectedGroupId = ref<string | null>(null)

const selectGroup = (id: string) => {
  selectedGroupId.value = id
  uiStore.scheduleView = 'schedule_detail'
}

const changeView = (view: string) => {
  uiStore.scheduleView = view
}
</script>
