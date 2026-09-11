<template>
  <!-- Calendar View -->
  <div
    v-if="uiStore.scheduleView === 'calendar'"
    class="flex h-full items-center justify-center"
    :class="uiStore.isNarrowScreen ? 'flex-col gap-4' : ''"
  >
    <div
      class="glass-effect flex flex-col overflow-hidden rounded-xl border border-cyan-500 shadow-sm"
      :class="uiStore.isNarrowScreen ? 'min-h-0 w-full flex-1' : 'w-2/3'"
    >
      <div class="flex items-center justify-between border-b border-cyan-500 p-4">
        <div class="flex w-full items-center justify-around">
          <button
            @click="changeMonth(-1)"
            class="rounded-lg p-2 text-cyan-500 transition-all duration-300 hover:bg-cyan-50"
          >
            <ChevronLeft />
          </button>
          <h3 class="text-brand text-lg font-bold">
            {{ $t("ui.calendarPage.yearMonth", { year: calendarYear, month: calendarMonth + 1 }) }}
          </h3>
          <button
            @click="changeMonth(1)"
            class="rounded-lg p-2 text-cyan-500 transition-all duration-300 hover:bg-cyan-50"
          >
            <ChevronRight />
          </button>
        </div>
      </div>
      <div class="flex flex-1 flex-col">
        <div
          class="calendar-grid border-b border-cyan-500 bg-slate-50/30 py-3 text-center text-[10px]
            font-bold tracking-widest text-white"
        >
          <div v-for="d in weekDays" :key="d">
            {{ d }}
          </div>
        </div>
        <div class="calendar-grid flex-1">
          <div
            v-for="(day, idx) in calendarDays"
            :key="'day-' + idx"
            @click="selectDate(day)"
            :class="[
              `day-cell relative cursor-pointer border-r border-b border-cyan-500 p-2 transition-all
              hover:bg-cyan-50/30`,
              !day.currentMonth ? 'bg-slate-50/20 opacity-30' : '',
            ]"
          >
            <span
              :class="[
                'text-sm font-medium',
                day.today
                  ? 'flex h-7 w-7 items-center justify-center rounded-full bg-cyan-500 text-white'
                  : 'text-white',
              ]"
              >{{ day.date }}</span
            >
            <div class="mt-1 space-y-1">
              <div
                v-for="event in getEvents(day)"
                :key="'event-' + event.id"
                class="truncate rounded bg-cyan-100 px-1.5 py-0.5 text-[9px] font-bold
                  text-cyan-700"
              >
                {{ event.title }}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
    <div
      class="glass-effect flex flex-col rounded-xl border border-cyan-500 shadow-sm"
      :class="uiStore.isNarrowScreen ? 'min-h-0 w-full' : 'h-full w-1/3 pl-4'"
    >
      <div :class="uiStore.isNarrowScreen ? 'p-3' : 'flex h-full flex-col p-4'">
        <h3 class="text-brand mb-4 text-lg font-bold">{{ $t("ui.calendarPage.title") }}</h3>

        <!-- 添加新事件按钮 -->
        <button
          @click="showAddEventModal = true"
          class="mb-4 flex w-full items-center justify-center rounded-lg bg-cyan-500 px-4 py-2
            text-white transition-all duration-300 hover:bg-cyan-600"
        >
          <span class="mr-2">+</span> {{ $t("ui.calendarPage.add") }}
        </button>

        <!-- 事件列表 -->
        <div class="flex-1 space-y-2 overflow-y-auto">
          <div
            v-for="event in sortedEvents"
            :key="event.id"
            class="cursor-pointer rounded-lg border border-cyan-500/30 bg-slate-50/30 p-3
              transition-all duration-300 hover:bg-slate-50/50"
            @click="selectEvent(event)"
          >
            <div class="flex items-start justify-between">
              <div class="flex-1">
                <h4 class="text-brand font-medium">{{ event.title }}</h4>
                <p class="mt-1 text-xs text-white">{{ formatDate(event.date) }}</p>
                <p v-if="event.desc" class="mt-1 text-xs text-white">{{ event.desc }}</p>
              </div>
              <button
                @click.stop="deleteEvent(event.id)"
                class="ml-2 text-red-500 hover:text-red-700"
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
          <div v-if="sortedEvents.length === 0" class="py-8 text-center text-gray-500">
            {{ $t("ui.calendarPage.empty") }}
          </div>
        </div>

        <!-- 添加事件模态框 -->
        <div
          v-if="showAddEventModal"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          @click.self="showAddEventModal = false"
        >
          <div class="glass-effect w-96 rounded-xl border border-cyan-500 p-6 shadow-sm">
            <h3 class="text-brand mb-4 text-lg font-bold">{{ $t("ui.calendarPage.add") }}</h3>

            <div class="space-y-4">
              <div>
                <label class="mb-1 block text-sm font-medium text-gray-700">{{
                  $t("ui.calendarPage.titleLabel")
                }}</label>
                <input
                  v-model="newEvent.title"
                  type="text"
                  class="w-full rounded-lg border border-cyan-500 px-3 py-2 focus:ring-2
                    focus:ring-cyan-500 focus:outline-none"
                  :placeholder="$t('ui.calendarPage.titlePlaceholder')"
                />
              </div>

              <div>
                <label class="mb-1 block text-sm font-medium text-gray-700">{{
                  $t("ui.calendarPage.dateLabel")
                }}</label>
                <input
                  v-model="newEvent.date"
                  type="date"
                  class="w-full rounded-lg border border-cyan-500 px-3 py-2 focus:ring-2
                    focus:ring-cyan-500 focus:outline-none"
                />
              </div>

              <div>
                <label class="mb-1 block text-sm font-medium text-gray-700">{{
                  $t("ui.calendarPage.descLabel")
                }}</label>
                <textarea
                  v-model="newEvent.desc"
                  class="w-full rounded-lg border border-cyan-500 px-3 py-2 focus:ring-2
                    focus:ring-cyan-500 focus:outline-none"
                  rows="3"
                  :placeholder="$t('ui.calendarPage.descPlaceholder')"
                ></textarea>
              </div>

              <div>
                <label class="mb-1 block text-sm font-medium text-gray-700">{{
                  $t("ui.calendarPage.cycleLabel")
                }}</label>
                <select
                  v-model="newEvent.cycle"
                  class="w-full rounded-lg border border-cyan-500 px-3 py-2 focus:ring-2
                    focus:ring-cyan-500 focus:outline-none"
                >
                  <option value="">{{ $t("ui.calendarPage.cycleNone") }}</option>
                  <option value="yearly">{{ $t("ui.calendarPage.cycleYearly") }}</option>
                  <option value="monthly">{{ $t("ui.calendarPage.cycleMonthly") }}</option>
                  <option value="weekly">{{ $t("ui.calendarPage.cycleWeekly") }}</option>
                </select>
              </div>
            </div>

            <div class="mt-6 flex justify-end space-x-2">
              <button
                @click="showAddEventModal = false"
                class="rounded-lg border border-cyan-500 px-4 py-2 text-cyan-500 transition-all
                  duration-300 hover:bg-cyan-50"
              >
                {{ $t("ui.calendarPage.cancel") }}
              </button>
              <button
                @click="addEvent"
                class="rounded-lg bg-cyan-500 px-4 py-2 text-white transition-all duration-300
                  hover:bg-cyan-600"
              >
                {{ $t("ui.calendarPage.addConfirm") }}
              </button>
            </div>
          </div>
        </div>

        <!-- 事件详情模态框 -->
        <div
          v-if="selectedEvent"
          class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          @click.self="selectedEvent = null"
        >
          <div class="glass-effect w-96 rounded-xl border border-cyan-500 p-6 shadow-sm">
            <h3 class="text-brand mb-4 text-lg font-bold">{{ selectedEvent.title }}</h3>

            <div class="space-y-2">
              <div>
                <span class="text-sm font-medium text-gray-700">{{
                  $t("ui.calendarPage.detailDateLabel")
                }}</span>
                <span class="text-sm">{{ formatDate(selectedEvent.date) }}</span>
              </div>

              <div v-if="selectedEvent.desc">
                <span class="text-sm font-medium text-gray-700">{{
                  $t("ui.calendarPage.detailDescLabel")
                }}</span>
                <p class="mt-1 text-sm">{{ selectedEvent.desc }}</p>
              </div>

              <div v-if="selectedEvent.cycle">
                <span class="text-sm font-medium text-gray-700">{{
                  $t("ui.calendarPage.detailCycleLabel")
                }}</span>
                <span class="text-sm">{{ getCycleText(selectedEvent.cycle) }}</span>
              </div>
            </div>

            <div class="mt-6 flex justify-end">
              <button
                @click="selectedEvent = null"
                class="rounded-lg bg-cyan-500 px-4 py-2 text-white transition-all duration-300
                  hover:bg-cyan-600"
              >
                {{ $t("ui.calendarPage.close") }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, watch, onMounted } from "vue";
  import { useI18n } from "vue-i18n";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { ChevronRight, ChevronLeft } from "lucide-vue-next";
  import { getSchedules, saveSchedules } from "@/api/services/schedule";

  const { t } = useI18n();
  const uiStore = useUIStore();

  // 数据存储
  interface ImportantDay {
    id: string;
    date: string;
    title: string;
    desc?: string;
    cycle?: string; // 日期周期
  }
  interface Day {
    date: number;
    month: number;
    year: number;
    currentMonth: boolean;
  }

  const importantDays = ref<ImportantDay[]>([]);
  const calendarInitialized = ref(false);
  const preventFirstSave = ref(true);

  const loadData = async () => {
    try {
      const data = await getSchedules();
      // 注意：这里要确保 data.importantDays 存在，否则给空数组
      importantDays.value = data.importantDays || [];
      calendarInitialized.value = true;
    } catch (e) {
      console.error("Failed to load calendar events", e);
    }
  };

  // 3. 监听并保存
  watch(
    importantDays,
    async (newVal) => {
      if (!calendarInitialized.value) return;
      if (preventFirstSave.value) {
        preventFirstSave.value = false;
        return;
      }
      try {
        await saveSchedules({ importantDays: newVal });
      } catch (e) {
        console.error("Failed to save calendar events", e);
      }
    },
    { deep: true }
  );

  onMounted(() => {
    loadData();
  });

  // Calendar Logic
  const calendarDate = ref(new Date());
  const calendarYear = computed(() => calendarDate.value.getFullYear());
  const calendarMonth = computed(() => calendarDate.value.getMonth());
  const weekDays = computed(() => [
    t("ui.calendarPage.week.sun"),
    t("ui.calendarPage.week.mon"),
    t("ui.calendarPage.week.tue"),
    t("ui.calendarPage.week.wed"),
    t("ui.calendarPage.week.thu"),
    t("ui.calendarPage.week.fri"),
    t("ui.calendarPage.week.sat"),
  ]);
  const selectedDate = ref<Day | null>(null);

  const calendarDays = computed(() => {
    const days = [];
    const firstDay = new Date(calendarYear.value, calendarMonth.value, 1);
    const lastDay = new Date(calendarYear.value, calendarMonth.value + 1, 0);
    const prevLastDay = new Date(calendarYear.value, calendarMonth.value, 0).getDate();

    for (let i = firstDay.getDay(); i > 0; i--) {
      days.push({
        date: prevLastDay - i + 1,
        month: calendarMonth.value - 1,
        year: calendarYear.value,
        currentMonth: false,
      });
    }
    const today = new Date();
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
      });
    }
    const remaining = 42 - days.length;
    for (let i = 1; i <= remaining; i++) {
      days.push({
        date: i,
        month: calendarMonth.value + 1,
        year: calendarYear.value,
        currentMonth: false,
      });
    }
    return days;
  });

  const changeMonth = (offset: number) => {
    calendarDate.value = new Date(calendarYear.value, calendarMonth.value + offset, 1);
  };
  const selectDate = (day: Day) => {
    selectedDate.value = day;
    // openModal()
  };
  const getEvents = (day: Day) => {
    const ds = `${day.year}-${String(day.month + 1).padStart(
      2,
      "0"
    )}-${String(day.date).padStart(2, "0")}`;
    return importantDays.value.filter((e) => e.date === ds);
  };

  // 排序后的事件列表（按日期升序）
  const sortedEvents = computed(() => {
    return [...importantDays.value].sort(
      (a, b) => new Date(a.date).getTime() - new Date(b.date).getTime()
    );
  });

  // 格式化日期
  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    return t("ui.calendarPage.fullDate", {
      year: date.getFullYear(),
      month: date.getMonth() + 1,
      day: date.getDate(),
    });
  };

  // 获取周期文本
  const getCycleText = (cycle: string) => {
    const cycleMap: { [key: string]: string } = {
      yearly: t("ui.calendarPage.cycleYearly"),
      monthly: t("ui.calendarPage.cycleMonthly"),
      weekly: t("ui.calendarPage.cycleWeekly"),
    };
    return cycleMap[cycle] || cycle;
  };

  // 添加事件相关
  const showAddEventModal = ref(false);
  const selectedEvent = ref<ImportantDay | null>(null);
  const newEvent = ref<ImportantDay>({
    id: "",
    date: "",
    title: "",
    desc: "",
    cycle: "",
  });

  // 添加事件
  const addEvent = () => {
    if (!newEvent.value.title || !newEvent.value.date) return;

    const id = Date.now().toString();
    importantDays.value.push({
      ...newEvent.value,
      id,
    });

    // 重置表单
    newEvent.value = {
      id: "",
      date: "",
      title: "",
      desc: "",
      cycle: "",
    };

    showAddEventModal.value = false;
  };

  // 删除事件
  const deleteEvent = (id: string) => {
    importantDays.value = importantDays.value.filter((e) => e.id !== id);
  };

  // 选择事件
  const selectEvent = (event: ImportantDay) => {
    selectedEvent.value = event;
  };

  const handleCreate = () => {
    showAddEventModal.value = true;
  };

  defineExpose({
    handleCreate,
  });
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
