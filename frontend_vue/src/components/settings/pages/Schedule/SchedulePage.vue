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

  <!-- 引入通用模态框 -->
  <BaseModal
    :show="showModal"
    :title="modalTitle"
    @close="showModal = false"
    @confirm="confirmCreate"
  >
    <!-- 场景1：新建日程组 -->
    <template v-if="uiStore.scheduleView === 'schedule_groups'">
      <input
        v-model="formData.groupTitle"
        placeholder="主题名称"
        class="w-full px-5 py-4 rounded-2xl border-none bg-slate-100 outline-none focus:ring-2 focus:ring-cyan-500/50 transition-all"
      />
      <textarea
        v-model="formData.groupDesc"
        placeholder="描述..."
        rows="3"
        class="w-full px-5 py-4 rounded-2xl border-none bg-slate-100 outline-none resize-none focus:ring-2 focus:ring-cyan-500/50 transition-all"
      ></textarea>
    </template>

    <!-- 场景2：新建日程项 -->
    <template v-else>
      <input
        v-model="formData.itemName"
        placeholder="活动名称"
        class="w-full px-5 py-4 rounded-2xl border-none bg-slate-100 outline-none focus:ring-2 focus:ring-cyan-500/50 transition-all"
      />
      <input
        v-model="formData.itemTime"
        type="time"
        class="w-full px-5 py-4 rounded-2xl border-none bg-slate-100 outline-none focus:ring-2 focus:ring-cyan-500/50 transition-all"
      />
      <textarea
        v-model="formData.itemContent"
        placeholder="指令详情..."
        rows="2"
        class="w-full px-5 py-4 rounded-2xl border-none bg-slate-100 outline-none resize-none focus:ring-2 focus:ring-cyan-500/50 transition-all"
      ></textarea>
    </template>
  </BaseModal>
</template>

<script setup lang="ts">
import { ref, computed, reactive, watch, onMounted } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { ArrowRight, Trash2, FolderKanban } from 'lucide-vue-next'
import { getSchedules, saveSchedules } from '@/api/services/schedule'

import BaseModal from '@/components/ui/BaseModal.vue'

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

const scheduleGroups = ref<Record<string, ScheduleGroup>>({})

const loadData = async () => {
  try {
    const data = await getSchedules()
    if (data && data.scheduleGroups) {
      scheduleGroups.value = data.scheduleGroups
    }
  } catch (e) {
    console.error('Failed to load schedules', e)
  }
}

watch(
  scheduleGroups,
  async (newVal) => {
    try {
      // 只发送 scheduleGroups 字段，后端只会更新这一部分
      await saveSchedules({ scheduleGroups: newVal })
    } catch (e) {
      console.error('Failed to save schedules', e)
    }
  },
  { deep: true },
)

onMounted(() => {
  loadData()
})

const activeGroup = computed(() => {
  if (!selectedGroupId.value) {
    return { items: [] }
  }
  return scheduleGroups.value[selectedGroupId.value] || { items: [] }
})

const removeScheduleItem = (idx: number) => {
  activeGroup.value.items.splice(idx, 1)
}

const selectedGroupId = ref<string | null>(null)

const selectGroup = (id: string) => {
  selectedGroupId.value = id
  uiStore.scheduleView = 'schedule_detail'
}

// 模态框状态
const showModal = ref(false)
const formData = reactive({
  groupTitle: '',
  groupDesc: '',
  itemName: '',
  itemTime: '',
  itemContent: '',
})

// 动态标题
const modalTitle = computed(() => {
  return uiStore.scheduleView === 'schedule_groups' ? '新建日程主题' : '新建具体日程'
})

// 父组件调用的方法
const handleCreate = () => {
  // 重置表单
  formData.groupTitle = ''
  formData.groupDesc = ''
  formData.itemName = ''
  formData.itemTime = ''
  formData.itemContent = ''

  showModal.value = true
}

// 确认创建逻辑
const confirmCreate = () => {
  if (uiStore.scheduleView === 'schedule_groups') {
    // 创建主题逻辑
    const newId = 'g' + Date.now()
    scheduleGroups.value[newId] = {
      title: formData.groupTitle,
      description: formData.groupDesc,
      items: [],
    }
  } else if (selectedGroupId.value) {
    // 创建日程项逻辑
    const group = scheduleGroups.value[selectedGroupId.value]
    if (group) {
      group.items.push({
        name: formData.itemName,
        time: formData.itemTime,
        content: formData.itemContent,
      })
    }
  }
  showModal.value = false
}

defineExpose({ handleCreate })
</script>
