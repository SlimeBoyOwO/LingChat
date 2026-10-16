<template>
  <!-- 窗口背景必须自己画满：App.vue 把 html/body 设成了 transparent，不画就透出桌面 -->
  <div
    class="flex h-full w-full flex-col overflow-hidden text-white select-none"
    style="background: linear-gradient(160deg, #171722 0%, #0e0e15 100%)"
  >
    <!-- 顶部菜单：日程 / 待办。整条兼作标题栏 -->
    <header class="flex shrink-0 items-center gap-1 border-b border-white/10 px-2 py-2">
      <button
        v-for="item in tabs"
        :key="item.key"
        @click="tab = item.key"
        class="flex items-center gap-1 rounded-lg px-2.5 py-1.5 text-base font-bold transition-colors"
        :class="
          tab === item.key
            ? 'text-brand bg-white/10'
            : 'text-white/50 hover:bg-white/5 hover:text-white/80'
        "
      >
        <component :is="item.icon" :size="14" />
        <span>{{ item.label }}</span>
      </button>

      <!-- 拖动区：只在这块空白上挂 data-tauri-drag-region，免得抢走两侧按钮的点击 -->
      <div class="h-full min-h-6 flex-1" data-tauri-drag-region></div>

      <button
        @click="toggleOnTop"
        class="rounded-lg p-1.5 transition-colors"
        :class="onTop ? 'text-brand hover:bg-white/10' : 'text-white/35 hover:bg-white/10'"
        :title="onTop ? $t('ui.todoWidget.unpin') : $t('ui.todoWidget.pin')"
      >
        <Pin v-if="onTop" :size="14" />
        <PinOff v-else :size="14" />
      </button>
      <button
        @click="closeWidget"
        class="rounded-lg p-1.5 text-white/50 transition-colors hover:bg-white/10 hover:text-white"
        :title="$t('ui.todoWidget.close')"
      >
        <X :size="14" />
      </button>
    </header>

    <div class="flex min-h-0 flex-1 flex-col gap-2.5 p-3">
      <!-- 最近一项的倒计时 -->
      <section class="glass-effect shrink-0 rounded-2xl p-3">
        <template v-if="countdown">
          <div class="flex items-center gap-1.5 text-sm font-bold text-white/45">
            <component :is="countdown.icon" :size="12" />
            <span class="min-w-0 truncate">{{ countdown.groupTitle }}</span>
            <span v-if="countdown.tomorrow" class="text-brand shrink-0">{{
              $t("ui.todoWidget.tomorrow")
            }}</span>
          </div>
          <p class="mt-1 truncate text-base font-bold text-white">{{ countdown.title }}</p>
          <p
            class="mt-1.5 font-mono text-2xl leading-none font-black tabular-nums"
            :class="countdown.overdue ? 'text-red-400' : 'text-brand'"
          >
            <span v-if="countdown.overdue" class="mr-1 font-sans text-sm">{{
              $t("ui.todoWidget.overdue")
            }}</span>
            {{ formatCountdown(countdown.diff) }}
          </p>
        </template>
        <p v-else class="py-1 text-center text-base text-white/40">{{ emptyHint }}</p>
      </section>

      <!-- 简化列表 -->
      <section class="min-h-0 flex-1 overflow-y-auto">
        <template v-if="tab === 'schedule'">
          <div
            v-for="entry in scheduleEntries"
            :key="entry.key"
            class="mb-1.5 flex items-center gap-2.5 rounded-xl px-2.5 py-2 transition-colors"
            :class="entry.passed ? 'opacity-35' : 'bg-white/5'"
          >
            <span
              class="text-brand shrink-0 rounded-md bg-cyan-500/20 px-1.5 py-0.5 font-mono text-sm font-bold"
              >{{ entry.time }}</span
            >
            <span class="min-w-0 flex-1 truncate text-base">{{ entry.name }}</span>
            <span class="shrink-0 truncate text-sm text-white/30">{{ entry.groupTitle }}</span>
          </div>
        </template>

        <template v-else>
          <div
            v-for="todo in pendingTodos"
            :key="todo.id"
            class="mb-1.5 flex items-center gap-2.5 rounded-xl bg-white/5 px-2.5 py-2"
          >
            <button
              @click="completeTodo(todo)"
              class="hover:border-brand h-5 w-5 shrink-0 rounded-md border-2 border-cyan-500/40 transition-all"
            ></button>
            <div class="min-w-0 flex-1">
              <p class="truncate text-base">{{ todo.text }}</p>
              <p
                v-if="todo.deadline"
                class="flex items-center gap-1 text-sm font-bold"
                :class="todo.overdue ? 'text-red-400' : 'text-brand'"
              >
                <CalendarClock :size="11" />
                <span>{{ formatDeadlineLabel(todo.deadline) }}</span>
              </p>
            </div>
            <span class="shrink-0 truncate text-sm text-white/30">{{ todo.groupTitle }}</span>
          </div>
        </template>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CalendarClock, CheckCircle2, Layers, Pin, PinOff, X } from "lucide-vue-next";
import { getSchedules, saveSchedules } from "@/api/services/schedule";
import {
  closeTodoWidget,
  isTodoWidgetOnTop,
  setTodoWidgetOnTop,
  shutdownTodoWidget,
  trackTodoWidgetGeometry,
} from "@/utils/todoWidget";

const { t } = useI18n();

interface WidgetTodo {
  id: number;
  text: string;
  priority: number;
  completed: boolean;
  deadline?: string | null;
}
interface WidgetTodoGroup {
  title: string;
  todos: WidgetTodo[];
}
interface WidgetScheduleItem {
  name: string;
  time: string;
  content?: string;
}
interface WidgetScheduleGroup {
  title: string;
  items: WidgetScheduleItem[];
}

type TabKey = "schedule" | "todo";

const tab = ref<TabKey>("todo");
const onTop = ref(isTodoWidgetOnTop());
const todoGroups = ref<Record<string, WidgetTodoGroup>>({});
const scheduleGroups = ref<Record<string, WidgetScheduleGroup>>({});
/** 每秒重算而非自减：窗口被节流后回来仍是对的（同 StatusInfoBar 的做法） */
const nowMs = ref(Date.now());

const tabs = computed(() => [
  { key: "schedule" as TabKey, label: t("ui.todoWidget.tabSchedule"), icon: Layers },
  { key: "todo" as TabKey, label: t("ui.todoWidget.tabTodo"), icon: CheckCircle2 },
]);

/** 归一化为 `YYYY-MM-DDTHH:mm` 并按本地时间解析，兼容 AI 写入的自由字符串 */
const parseDeadlineMs = (raw?: string | null): number | null => {
  if (!raw) return null;
  const normalized = raw.trim().replace(" ", "T").slice(0, 16);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(normalized)) return null;
  const at = new Date(normalized).getTime();
  return Number.isNaN(at) ? null : at;
};

const formatDeadlineLabel = (raw?: string | null): string => {
  if (!raw) return "";
  return raw.trim().replace(" ", "T").slice(5, 16).replace("T", " ");
};

const parseMinutes = (time?: string | null): number | null => {
  const matched = /^(\d{1,2}):(\d{2})/.exec((time ?? "").trim());
  if (!matched) return null;
  const hours = Number(matched[1]);
  const minutes = Number(matched[2]);
  if (hours > 23 || minutes > 59) return null;
  return hours * 60 + minutes;
};

const formatCountdown = (diff: number): string => {
  const totalSeconds = Math.floor(Math.abs(diff) / 1000);
  const days = Math.floor(totalSeconds / 86400);
  const pad = (value: number) => String(value).padStart(2, "0");
  const clock = `${pad(Math.floor((totalSeconds % 86400) / 3600))}:${pad(
    Math.floor((totalSeconds % 3600) / 60),
  )}:${pad(totalSeconds % 60)}`;
  return days > 0 ? `${days}${t("ui.todoWidget.dayUnit")} ${clock}` : clock;
};

/** 全部未完成待办：有截止时间的按时间升序在前，没截止的按优先级排在后面 */
const pendingTodos = computed(() => {
  const list: Array<WidgetTodo & { groupTitle: string; gid: string; due: number | null }> = [];
  Object.entries(todoGroups.value).forEach(([gid, group]) => {
    group.todos?.forEach((todo) => {
      if (todo.completed) return;
      list.push({ ...todo, groupTitle: group.title, gid, due: parseDeadlineMs(todo.deadline) });
    });
  });
  return list
    .sort((a, b) => {
      if (a.due === null && b.due === null) return b.priority - a.priority;
      if (a.due === null) return 1;
      if (b.due === null) return -1;
      return a.due - b.due;
    })
    .map((todo) => ({ ...todo, overdue: todo.due !== null && todo.due < nowMs.value }));
});

/** 全部日程条目按一天内的时刻升序；passed 表示今天这一趟已经过去了 */
const scheduleEntries = computed(() => {
  const now = nowMs.value;
  const dayStart = new Date(now);
  dayStart.setHours(0, 0, 0, 0);
  const base = dayStart.getTime();
  const list: Array<{
    key: string;
    name: string;
    time: string;
    groupTitle: string;
    minutes: number;
  }> = [];
  Object.entries(scheduleGroups.value).forEach(([gid, group]) => {
    group.items?.forEach((item, index) => {
      const minutes = parseMinutes(item.time);
      if (minutes === null) return;
      list.push({
        key: `${gid}-${index}`,
        name: item.name,
        time: item.time.slice(0, 5),
        groupTitle: group.title,
        minutes,
      });
    });
  });
  return list
    .sort((a, b) => a.minutes - b.minutes)
    .map((entry) => ({ ...entry, passed: base + entry.minutes * 60000 < now }));
});

/** 倒计时卡没有目标时的说明；待办有内容但都没设截止，与「没有待办」要分开说 */
const emptyHint = computed(() => {
  if (tab.value === "schedule") return t("ui.todoWidget.emptySchedule");
  return pendingTodos.value.length === 0
    ? t("ui.todoWidget.emptyTodo")
    : t("ui.todoWidget.noDeadlineTodo");
});

/**
 * 倒计时目标。
 *
 * 日程条目只有 HH:MM、按天重复，没有日期，所以目标取今天下一个不早于此刻的时刻；
 * 今天都过完了就顺延到明天最早的一条，并标注「明天」。
 */
const countdown = computed(() => {
  const now = nowMs.value;
  if (tab.value === "todo") {
    const next = pendingTodos.value.find((todo) => todo.due !== null);
    if (!next || next.due === null) return null;
    return {
      title: next.text,
      groupTitle: next.groupTitle,
      icon: CalendarClock,
      tomorrow: false,
      overdue: next.due < now,
      diff: next.due - now,
    };
  }

  if (scheduleEntries.value.length === 0) return null;
  const dayStart = new Date(now);
  dayStart.setHours(0, 0, 0, 0);
  const base = dayStart.getTime();
  const upcoming = scheduleEntries.value.find((entry) => base + entry.minutes * 60000 >= now);
  const target = upcoming ?? scheduleEntries.value[0];
  // 顺延到明天时不能用 base + 86400000，夏令时那天会差一小时
  const tomorrowStart = new Date(dayStart);
  tomorrowStart.setDate(tomorrowStart.getDate() + 1);
  const targetMs = base + target.minutes * 60000;
  const at = upcoming ? targetMs : tomorrowStart.getTime() + target.minutes * 60000;
  return {
    title: target.name,
    groupTitle: target.groupTitle,
    icon: Layers,
    tomorrow: !upcoming,
    overdue: false,
    diff: at - now,
  };
});

const loadData = async () => {
  try {
    const data = await getSchedules();
    todoGroups.value = (data.todoGroups ?? {}) as Record<string, WidgetTodoGroup>;
    scheduleGroups.value = (data.scheduleGroups ?? {}) as Record<string, WidgetScheduleGroup>;
  } catch (e) {
    console.error("[TodoWidget] 读取日程数据失败:", e);
  }
};

const completeTodo = async (todo: WidgetTodo & { gid: string }) => {
  try {
    // save_schedules 收到 todoGroups 是整块覆盖，先拉一次最新的再改这一条，
    // 免得用陈旧快照盖掉别处（主面板或 AI）刚写入的改动
    const fresh = await getSchedules();
    const groups = (fresh.todoGroups ?? {}) as Record<string, WidgetTodoGroup>;
    const target = groups[todo.gid]?.todos?.find((item) => item.id === todo.id);
    if (!target) return;
    target.completed = true;
    await saveSchedules({ todoGroups: groups });
    todoGroups.value = groups;
  } catch (e) {
    console.error("[TodoWidget] 勾选待办失败:", e);
  }
};

const toggleOnTop = async () => {
  const next = !onTop.value;
  onTop.value = next;
  setTodoWidgetOnTop(next);
  try {
    await getCurrentWindow().setAlwaysOnTop(next);
  } catch (e) {
    console.error("[TodoWidget] 切换置顶失败:", e);
  }
};

// 关闭统一走 close-requested 收口（见下面的 onCloseRequested）：自己的 X 按钮与
// Alt+F4 都从那里同步状态给主窗口，这里不重复处理
const closeWidget = () => void closeTodoWidget();

let ticker: ReturnType<typeof setInterval> | null = null;
let unlistenSchedules: UnlistenFn | null = null;
let unlistenClose: UnlistenFn | null = null;
let stopTrackingGeometry: (() => void) | null = null;

onMounted(async () => {
  await loadData();
  ticker = setInterval(() => {
    nowMs.value = Date.now();
  }, 1000);
  unlistenSchedules = await listen("schedules:changed", () => void loadData());
  stopTrackingGeometry = await trackTodoWidgetGeometry();
  unlistenClose = await getCurrentWindow().onCloseRequested(async (event) => {
    // 先拦下关闭，把状态广播出去，再自己销毁；否则窗口会先被拆掉、主窗口收不到
    event.preventDefault();
    await shutdownTodoWidget();
  });
});

onUnmounted(() => {
  if (ticker !== null) clearInterval(ticker);
  unlistenSchedules?.();
  unlistenClose?.();
  stopTrackingGeometry?.();
});
</script>
