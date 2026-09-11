<template>
  <div class="flex w-full flex-1 flex-col overflow-hidden md:flex-row" :class="containerClass">
    <!-- 导航菜单 (左侧)：宽屏始终可见；窄屏仅在浏览菜单层级时可见 -->
    <aside
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'menu'"
      class="flex w-full flex-col border-r border-cyan-300 p-6 md:w-64"
      :class="{ 'min-h-0 flex-1': uiStore.isNarrowScreen }"
    >
      <div
        class="text-brand inset_0_1px_1px_rgba(255,255,255,0.1)] mb-8 flex items-center space-x-3
          rounded-lg px-3.75 py-2.5 text-base font-bold"
      >
        <div class="relative">
          <div
            class="flex h-10 w-10 items-center justify-center rounded-xl bg-cyan-500 text-white
              shadow-lg"
          >
            <Sparkles :size="20" />
          </div>
        </div>
        <h1 class="text-xl font-bold tracking-tight text-white">LingChat AI</h1>
      </div>

      <nav class="min-h-0 w-full flex-1 space-y-2 overflow-y-auto">
        <button
          class="adv-nav-link relative z-10 flex w-full items-center space-x-6 rounded-lg px-5 py-3
            text-white no-underline transition-colors duration-200 hover:bg-gray-200
            hover:text-black active:font-bold active:text-white"
          @click="changeView('schedule_groups')"
        >
          <Layers :size="18" />
          <span>{{ $t("ui.scheduleContent.navSchedule") }}</span>
        </button>
        <button
          class="adv-nav-link relative z-10 flex w-full items-center space-x-6 rounded-lg px-5 py-3
            text-white no-underline transition-colors duration-200 hover:bg-gray-200
            hover:text-black active:font-bold active:text-white"
          @click="changeView('todo_groups')"
        >
          <CheckCircle2 :size="18" />
          <span>{{ $t("ui.scheduleContent.navTodo") }}</span>
        </button>
        <button
          class="adv-nav-link relative z-10 flex w-full items-center space-x-6 rounded-lg px-5 py-3
            text-white no-underline transition-colors duration-200 hover:bg-gray-200
            hover:text-black active:font-bold active:text-white"
          @click="changeView('calendar')"
        >
          <CalendarDays :size="18" />
          <span>{{ $t("ui.scheduleContent.navCalendar") }}</span>
        </button>
        <button
          class="adv-nav-link relative z-10 flex w-full items-center space-x-6 rounded-lg px-5 py-3
            text-white no-underline transition-colors duration-200 hover:bg-gray-200
            hover:text-black active:font-bold active:text-white"
          @click="changeView('proactive_settings')"
        >
          <Cat :size="18" />
          <span>{{ $t("ui.scheduleContent.navProactive") }}</span>
        </button>
        <button
          class="adv-nav-link relative z-10 flex w-full items-center space-x-6 rounded-lg px-5 py-3
            text-white no-underline transition-colors duration-200 hover:bg-gray-200
            hover:text-black active:font-bold active:text-white"
          @click="changeView('tool_calls')"
        >
          <Wrench :size="18" />
          <span>{{ $t("ui.scheduleContent.navToolCalls") }}</span>
        </button>
      </nav>

      <div class="mt-auto mb-6 rounded-2xl border border-cyan-500/20 bg-cyan-50/10 p-4">
        <div class="text-brand mb-2 flex items-center text-xs font-bold">
          <span class="mr-2 h-2 w-2 animate-pulse rounded-full bg-cyan-500"></span>
          Ling Clock
        </div>
        <p class="text-xs leading-relaxed text-white italic">
          {{ $t("ui.scheduleContent.clockTip") }}
        </p>
      </div>
    </aside>

    <main
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'content'"
      class="flex w-full flex-1 flex-col overflow-hidden"
    >
      <header
        class="flex shrink-0 items-center justify-between border-b border-cyan-300"
        :class="uiStore.isNarrowScreen ? 'px-3 py-3' : 'mt-2 p-6'"
      >
        <div
          class="flex min-w-0 items-center"
          :class="uiStore.isNarrowScreen ? 'space-x-2' : 'space-x-4 pl-4'"
        >
          <!-- 窄屏：返回菜单按钮 -->
          <button
            v-if="uiStore.isNarrowScreen"
            @click="narrowViewLevel = 'menu'"
            class="flex shrink-0 items-center gap-1 rounded-lg px-1.5 py-1 text-sm text-white/70
              transition-colors hover:bg-white/10 hover:text-white"
          >
            <ChevronLeft :size="18" />
          </button>
          <!-- 宽屏：返回上级视图（详情 → 分组） -->
          <button
            v-show="
              !uiStore.isNarrowScreen &&
              (uiStore.scheduleView === 'schedule_detail' || uiStore.scheduleView === 'todo_detail')
            "
            @click="goBackToParentView"
            class="rounded-full p-2 text-cyan-600 transition-all hover:bg-cyan-50"
          >
            <ChevronLeft />
          </button>
          <div class="min-w-0">
            <h2
              class="text-brand truncate font-bold"
              :class="uiStore.isNarrowScreen ? 'text-base' : 'mb-2 text-2xl'"
            >
              {{ titleInfo.title }}
            </h2>
            <p v-show="!uiStore.isNarrowScreen" class="mt-0.5 text-xs tracking-wide text-white">
              {{ titleInfo.subtitle }}
            </p>
          </div>
        </div>

        <button
          v-show="
            !uiStore.scheduleView.startsWith('proactive') &&
            !uiStore.scheduleView.startsWith('tool_calls')
          "
          @click="triggerCreate"
          class="flex shrink-0 items-center rounded-xl bg-cyan-500 text-white shadow-lg
            transition-all hover:bg-cyan-600"
          :class="uiStore.isNarrowScreen ? 'space-x-1 px-3 py-2 text-sm' : 'space-x-2 px-5 py-2.5'"
        >
          <Plus :size="uiStore.isNarrowScreen ? 16 : undefined" />
          <span class="font-medium" :class="{ hidden: uiStore.isNarrowScreen }">{{
            $t("ui.scheduleContent.create")
          }}</span>
        </button>
      </header>

      <!-- 内容滚动容器 -->
      <div
        class="custom-scrollbar flex-1 overflow-y-auto"
        :class="uiStore.isNarrowScreen ? 'p-3' : 'p-6'"
      >
        <!--日程界面-->
        <SchedulePage ref="scheduleRef" />

        <!--待办事项界面-->
        <TodoPage ref="todoRef" />

        <!--日历页面-->
        <CalendarPage ref="calendarRef" />

        <ProactivePage ref="proactiveRef" />

        <!--工具调用设置界面-->
        <ToolCallsPage />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
  import { computed, ref } from "vue";
  import { useI18n } from "vue-i18n";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import TodoPage from "@/components/schedule/pages/TodoPage.vue";
  import SchedulePage from "@/components/schedule/pages/SchedulePage.vue";
  import CalendarPage from "@/components/schedule/pages/CalendarPage.vue";
  import ProactivePage from "@/components/schedule/pages/ProactivePage.vue";
  import ToolCallsPage from "@/components/schedule/pages/ToolCallsPage.vue";
  import {
    Layers,
    CheckCircle2,
    CalendarDays,
    Plus,
    Cat,
    ChevronLeft,
    Sparkles,
    Wrench,
  } from "lucide-vue-next";

  type Variant = "settings" | "popup";

  const props = withDefaults(
    defineProps<{
      variant?: Variant;
    }>(),
    { variant: "settings" }
  );

  const uiStore = useUIStore();
  const { t } = useI18n();
  const narrowViewLevel = ref<"menu" | "content">("menu");

  const scheduleRef = ref();
  const todoRef = ref();
  const calendarRef = ref();
  const titleInfo = computed(() => {
    const currentView = uiStore.scheduleView;

    if (currentView.startsWith("schedule")) {
      return {
        title: t("ui.scheduleContent.titleSchedule"),
        subtitle: t("ui.scheduleContent.subtitleSchedule"),
      };
    } else if (currentView.startsWith("todo")) {
      return {
        title: t("ui.scheduleContent.titleTodo"),
        subtitle: t("ui.scheduleContent.subtitleTodo"),
      };
    } else if (currentView.startsWith("proactive")) {
      return {
        title: t("ui.scheduleContent.titleProactive"),
        subtitle: t("ui.scheduleContent.subtitleProactive"),
      };
    } else if (currentView.startsWith("tool_calls")) {
      return {
        title: t("ui.scheduleContent.titleToolCalls"),
        subtitle: t("ui.scheduleContent.subtitleToolCalls"),
      };
    } else if (currentView.startsWith("calendar")) {
      return {
        title: t("ui.scheduleContent.titleCalendar"),
        subtitle: t("ui.scheduleContent.subtitleCalendar"),
      };
    } else {
      // 默认情况
      return {
        title: t("ui.scheduleContent.titleDefault"),
        subtitle: t("ui.scheduleContent.subtitleDefault"),
      };
    }
  });

  const triggerCreate = () => {
    const currentView = uiStore.scheduleView;

    // 这里的逻辑是：判断当前在哪个视图，就调用哪个组件内部的 handleCreate 方法
    if (currentView.startsWith("schedule")) {
      // 日程相关视图
      scheduleRef.value?.handleCreate();
    } else if (currentView.startsWith("todo")) {
      // 待办相关视图
      todoRef.value?.handleCreate();
    } else if (currentView === "calendar") {
      // 日历视图
      calendarRef.value?.handleCreate();
    }
  };

  const changeView = (view: string) => {
    uiStore.scheduleView = view;
    // 窄屏下自动切换到内容视图
    if (uiStore.isNarrowScreen) {
      narrowViewLevel.value = "content";
    }
  };

  const goBackToParentView = () => {
    if (uiStore.scheduleView === "schedule_detail") {
      uiStore.scheduleView = "schedule_groups";
    } else if (uiStore.scheduleView === "todo_detail") {
      uiStore.scheduleView = "todo_groups";
    }
  };

  const containerClass = computed(() => {
    // settings：沿用原来的全屏设置页布局
    if (props.variant === "settings") {
      return "h-[85dvh] max-w-6xl md:w-[calc(100vw-4rem)] glass-panel bg-white/10 rounded-2xl";
    }
    // popup：由父级 modal 控制尺寸和样式，此处填满容器
    return "w-full h-full";
  });
</script>
