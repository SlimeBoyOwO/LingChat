<template>
  <!-- Calendar View -->
  <div v-if="uiStore.scheduleView === 'calendar'" class="h-full flex items-center justify-center">
    <div
      class="w-2/3 glass-effect rounded-xl border border-cyan-500 shadow-sm overflow-hidden flex flex-col"
    >
      <div class="p-4 flex justify-between items-center border-b border-cyan-500">
        <div class="flex w-full justify-around items-center">
          <button
            @click="changeMonth(-1)"
            class="p-2 hover:bg-cyan-50 rounded-lg text-cyan-500 transition-all duration-300"
          >
            <ChevronLeft />
          </button>
          <h3 class="text-lg font-bold text-brand">
            {{ calendarYear }}年 {{ calendarMonth + 1 }}月
          </h3>
          <button
            @click="changeMonth(1)"
            class="p-2 hover:bg-cyan-50 rounded-lg text-cyan-500 transition-all duration-300"
          >
            <ChevronRight />
          </button>
        </div>
      </div>
      <div class="flex-1 flex flex-col">
        <div
          class="calendar-grid border-b border-cyan-500 bg-slate-50/30 font-bold text-[10px] text-white text-center py-3 tracking-widest"
        >
          <div v-for="d in ['日', '一', '二', '三', '四', '五', '六']" :key="d">
            {{ d }}
          </div>
        </div>
        <div class="calendar-grid flex-1">
          <div
            v-for="(day, idx) in calendarDays"
            :key="'day-' + idx"
            @click="selectDate(day)"
            :class="[
              'day-cell p-2 border-r border-b border-cyan-500 transition-all cursor-pointer relative hover:bg-cyan-50/30',
              !day.currentMonth ? 'bg-slate-50/20 opacity-30' : '',
            ]"
          >
            <span
              :class="[
                'text-sm font-medium',
                day.today
                  ? 'w-7 h-7 bg-cyan-500 text-white rounded-full flex items-center justify-center'
                  : 'text-white',
              ]"
              >{{ day.date }}</span
            >
            <div class="mt-1 space-y-1">
              <div
                v-for="event in getEvents(day)"
                :key="'event-' + event.id"
                class="text-[9px] bg-cyan-100 text-cyan-700 px-1.5 py-0.5 rounded truncate font-bold"
              >
                {{ event.title }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <div class="h-full w-1/3 pl-4">
      <div
        class="glass-effect rounded-xl border border-cyan-500 shadow-sm p-4 h-full flex flex-col"
      >
        <h3 class="text-lg font-bold text-brand mb-4">重要日子</h3>

        <!-- 添加新事件按钮 -->
        <button
          @click="showAddEventModal = true"
          class="w-full bg-cyan-500 text-white rounded-lg py-2 px-4 mb-4 hover:bg-cyan-600 transition-all duration-300 flex items-center justify-center"
        >
          <span class="mr-2">+</span> 添加重要日子
        </button>

        <!-- 事件列表 -->
        <div class="flex-1 overflow-y-auto space-y-2">
          <div
            v-for="event in sortedEvents"
            :key="event.id"
            class="p-3 bg-slate-50/30 rounded-lg border border-cyan-500/30 hover:bg-slate-50/50 transition-all duration-300 cursor-pointer"
            @click="selectEvent(event)"
          >
            <div class="flex justify-between items-start">
              <div class="flex-1">
                <h4 class="font-medium text-brand">{{ event.title }}</h4>
                <p class="text-xs text-white mt-1">{{ formatDate(event.date) }}</p>
                <p v-if="event.desc" class="text-xs text-white mt-1">{{ event.desc }}</p>
              </div>
              <button
                @click.stop="deleteEvent(event.id)"
                class="text-red-500 hover:text-red-700 ml-2"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  class="h-4 w-4"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>
          </div>

          <!-- 空状态 -->
          <div v-if="sortedEvents.length === 0" class="text-center py-8 text-gray-500">
            暂无重要日子
          </div>
        </div>

        <!-- 添加事件模态框 -->
        <div
          v-if="showAddEventModal"
          class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
          @click.self="showAddEventModal = false"
        >
          <div class="glass-effect rounded-xl border border-cyan-500 shadow-sm p-6 w-96">
            <h3 class="text-lg font-bold text-brand mb-4">添加重要日子</h3>

            <div class="space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">标题</label>
                <input
                  v-model="newEvent.title"
                  type="text"
                  class="w-full px-3 py-2 border border-cyan-500 rounded-lg focus:outline-none focus:ring-2 focus:ring-cyan-500"
                  placeholder="输入标题"
                />
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">日期</label>
                <input
                  v-model="newEvent.date"
                  type="date"
                  class="w-full px-3 py-2 border border-cyan-500 rounded-lg focus:outline-none focus:ring-2 focus:ring-cyan-500"
                />
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">描述 (可选)</label>
                <textarea
                  v-model="newEvent.desc"
                  class="w-full px-3 py-2 border border-cyan-500 rounded-lg focus:outline-none focus:ring-2 focus:ring-cyan-500"
                  rows="3"
                  placeholder="输入描述"
                ></textarea>
              </div>

              <div>
                <label class="block text-sm font-medium text-gray-700 mb-1">周期 (可选)</label>
                <select
                  v-model="newEvent.cycle"
                  class="w-full px-3 py-2 border border-cyan-500 rounded-lg focus:outline-none focus:ring-2 focus:ring-cyan-500"
                >
                  <option value="">无</option>
                  <option value="yearly">每年</option>
                  <option value="monthly">每月</option>
                  <option value="weekly">每周</option>
                </select>
              </div>
            </div>

            <div class="flex justify-end space-x-2 mt-6">
              <button
                @click="showAddEventModal = false"
                class="px-4 py-2 border border-cyan-500 text-cyan-500 rounded-lg hover:bg-cyan-50 transition-all duration-300"
              >
                取消
              </button>
              <button
                @click="addEvent"
                class="px-4 py-2 bg-cyan-500 text-white rounded-lg hover:bg-cyan-600 transition-all duration-300"
              >
                添加
              </button>
            </div>
          </div>
        </div>

        <!-- 事件详情模态框 -->
        <div
          v-if="selectedEvent"
          class="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
          @click.self="selectedEvent = null"
        >
          <div class="glass-effect rounded-xl border border-cyan-500 shadow-sm p-6 w-96">
            <h3 class="text-lg font-bold text-brand mb-4">{{ selectedEvent.title }}</h3>

            <div class="space-y-2">
              <div>
                <span class="text-sm font-medium text-gray-700">日期：</span>
                <span class="text-sm">{{ formatDate(selectedEvent.date) }}</span>
              </div>

              <div v-if="selectedEvent.desc">
                <span class="text-sm font-medium text-gray-700">描述：</span>
                <p class="text-sm mt-1">{{ selectedEvent.desc }}</p>
              </div>

              <div v-if="selectedEvent.cycle">
                <span class="text-sm font-medium text-gray-700">周期：</span>
                <span class="text-sm">{{ getCycleText(selectedEvent.cycle) }}</span>
              </div>
            </div>

            <div class="flex justify-end mt-6">
              <button
                @click="selectedEvent = null"
                class="px-4 py-2 bg-cyan-500 text-white rounded-lg hover:bg-cyan-600 transition-all duration-300"
              >
                关闭
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
import { ChevronRight, ChevronLeft } from 'lucide-vue-next'

const uiStore = useUIStore()

// 数据存储
interface ImportantDay {
  id: string
  date: string
  title: string
  desc?: string
  cycle?: string // 日期周期
}
interface Day {
  date: number
  month: number
  year: number
  currentMonth: boolean
}

const importantDays = ref<ImportantDay[]>([{ id: 'e1', date: '2025-11-09', title: '莱姆生日' }])

// Calendar Logic
const calendarDate = ref(new Date())
const calendarYear = computed(() => calendarDate.value.getFullYear())
const calendarMonth = computed(() => calendarDate.value.getMonth())
const selectedDate = ref<Day | null>(null)

const calendarDays = computed(() => {
  const days = []
  const firstDay = new Date(calendarYear.value, calendarMonth.value, 1)
  const lastDay = new Date(calendarYear.value, calendarMonth.value + 1, 0)
  const prevLastDay = new Date(calendarYear.value, calendarMonth.value, 0).getDate()

  for (let i = firstDay.getDay(); i > 0; i--) {
    days.push({
      date: prevLastDay - i + 1,
      month: calendarMonth.value - 1,
      year: calendarYear.value,
      currentMonth: false,
    })
  }
  const today = new Date()
  for (let i = 1; i <= lastDay.getDate(); i++) {
    days.push({
      date: i,
      month: calendarMonth.value,
      year: calendarYear.value,
      currentMonth: true,
      today:
        today.getDate() === i &&
        today.getMonth() === calendarMonth.value &&
        today.getFullYear() === calendarYear.value,
    })
  }
  const remaining = 42 - days.length
  for (let i = 1; i <= remaining; i++) {
    days.push({
      date: i,
      month: calendarMonth.value + 1,
      year: calendarYear.value,
      currentMonth: false,
    })
  }
  return days
})

const changeMonth = (offset: number) => {
  calendarDate.value = new Date(calendarYear.value, calendarMonth.value + offset, 1)
}
const selectDate = (day: Day) => {
  selectedDate.value = day
  // openModal()
}
const getEvents = (day: Day) => {
  const ds = `${day.year}-${String(day.month + 1).padStart(
    2,
    '0',
  )}-${String(day.date).padStart(2, '0')}`
  return importantDays.value.filter((e) => e.date === ds)
}

// 排序后的事件列表（按日期升序）
const sortedEvents = computed(() => {
  return [...importantDays.value].sort(
    (a, b) => new Date(a.date).getTime() - new Date(b.date).getTime(),
  )
})

// 格式化日期
const formatDate = (dateString: string) => {
  const date = new Date(dateString)
  return `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日`
}

// 获取周期文本
const getCycleText = (cycle: string) => {
  const cycleMap: { [key: string]: string } = {
    yearly: '每年',
    monthly: '每月',
    weekly: '每周',
  }
  return cycleMap[cycle] || cycle
}

// 添加事件相关
const showAddEventModal = ref(false)
const selectedEvent = ref<ImportantDay | null>(null)
const newEvent = ref<ImportantDay>({
  id: '',
  date: '',
  title: '',
  desc: '',
  cycle: '',
})

// 添加事件
const addEvent = () => {
  if (!newEvent.value.title || !newEvent.value.date) return

  const id = Date.now().toString()
  importantDays.value.push({
    ...newEvent.value,
    id,
  })

  // 重置表单
  newEvent.value = {
    id: '',
    date: '',
    title: '',
    desc: '',
    cycle: '',
  }

  showAddEventModal.value = false
}

// 删除事件
const deleteEvent = (id: string) => {
  importantDays.value = importantDays.value.filter((e) => e.id !== id)
}

// 选择事件
const selectEvent = (event: ImportantDay) => {
  selectedEvent.value = event
}
</script>

<style scoped>
.calendar-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
}
.day-cell {
  aspect-ratio: 1 / 1;
}
[v-cloak] {
  display: none;
}
</style>
