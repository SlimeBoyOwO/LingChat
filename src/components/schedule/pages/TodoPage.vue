<template>
  <div v-if="uiStore.scheduleView === 'todo_groups'" class="space-y-8">
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
      <div
        v-for="(group, id) in todoGroups"
        :key="'group-' + id"
        @click="selectTodoGroup(id)"
        class="glass-effect group relative flex cursor-pointer items-center justify-between rounded-2xl border border-slate-100 p-5 shadow-sm transition-all hover:border-cyan-200"
      >
        <button
          @click.stop="removeTodoGroup(id)"
          class="absolute top-1 left-1 z-10 p-1 text-slate-200 hover:text-red-400"
        >
          <Trash2 :size="18" />
        </button>
        <div class="ml-2 flex items-center space-x-4">
          <div
            class="flex h-10 w-10 items-center justify-center rounded-xl bg-cyan-500 text-cyan-50 transition-all group-hover:bg-cyan-50 group-hover:text-cyan-500"
          >
            <Folder />
          </div>
          <div>
            <h4 class="text-brand font-bold">
              {{ group.title }}
            </h4>
            <p class="text-[10px] font-bold text-white uppercase">
              {{ $t("ui.todoPage.taskCount", { count: group.todos.length }) }}
            </p>
          </div>
        </div>
        <ChevronRight class="text-slate-200 group-hover:text-cyan-500" />
      </div>
    </div>

    <!-- High Priority Global Tasks -->
    <div class="space-y-4">
      <h3 class="flex items-center text-xs font-black tracking-[0.2em] text-slate-50 uppercase">
        <Zap class="mr-2 h-3 w-3 text-amber-400" />
        {{ $t("ui.todoPage.globalPending") }}
      </h3>
      <div
        v-if="globalPendingTodos.length === 0"
        class="text-brand font-blod rounded-3xl border border-dashed border-slate-200 py-10 text-center text-xl"
      >
        {{ $t("ui.todoPage.noPending") }}
      </div>
      <div
        v-for="todo in globalPendingTodos"
        :key="'global-' + todo.id"
        @click="openEdit(todo)"
        class="glass-effect flex cursor-pointer items-center space-x-4 rounded-2xl border-l-4 border-l-cyan-500 p-4 shadow-sm"
      >
        <button
          @click.stop="completeTodo(todo)"
          class="h-6 w-6 rounded-lg border-2 border-cyan-100 transition-all hover:border-cyan-500"
        ></button>

        <div class="flex-1">
          <div class="flex items-center space-x-2">
            <span class="rounded bg-white/80 px-1.5 py-0.5 text-[11px] font-bold text-cyan-500">{{
              todo.groupTitle
            }}</span>
            <p class="font-bold text-cyan-50">{{ todo.text }}</p>
          </div>
          <div class="mt-1 flex items-center">
            <Star
              v-for="s in 5"
              :key="'star-global-' + todo.id + '-' + s"
              :class="[
                'h-3 w-3',
                s <= todo.priority ? 'fill-amber-400 text-amber-400' : 'text-slate-100',
              ]"
            />
          </div>
          <div
            v-if="todo.deadline"
            class="mt-1 flex items-center gap-1 text-[11px] font-bold"
            :class="isOverdue(todo) ? 'text-red-400' : 'text-cyan-500'"
          >
            <CalendarClock class="h-3 w-3" />
            <span>{{ formatDeadline(todo.deadline) }}</span>
            <BellRing v-if="todo.remindBeforeMinutes" class="h-3 w-3" />
          </div>
        </div>
      </div>
    </div>

    <!-- Global Completed History -->
    <div v-if="globalCompletedTodos.length > 0" class="space-y-3">
      <button
        @click="showCompleted = !showCompleted"
        class="flex items-center space-x-2 px-1 text-slate-400 transition-colors hover:text-cyan-600"
      >
        <component :is="showCompleted ? ChevronDown : ChevronRight" class="h-4 w-4" />
        <span class="text-[10px] font-black tracking-widest uppercase">{{
          $t("ui.todoPage.completedHistory", { count: globalCompletedTodos.length })
        }}</span>
      </button>
      <div v-if="showCompleted" class="space-y-2">
        <div
          v-for="todo in globalCompletedTodos"
          :key="'done-' + todo.id"
          class="flex items-center space-x-4 rounded-2xl border border-slate-100 bg-slate-50/50 p-4 opacity-50"
        >
          <CheckCircle class="h-5 w-5 text-cyan-500" />
          <div class="flex-1">
            <div class="flex items-center space-x-2">
              <span class="text-brand rounded border border-slate-200 px-1.5 py-0.5 text-[9px]">{{
                todo.groupTitle
              }}</span>
              <p class="text-sm text-gray-200 line-through">
                {{ todo.text }}
              </p>
            </div>
          </div>
          <button
            @click.stop="undoComplete(todo)"
            class="text-[10px] font-bold text-cyan-600 hover:underline"
          >
            {{ $t("ui.todoPage.undo") }}
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- Todo Detail View -->
  <div v-if="uiStore.scheduleView === 'todo_detail'" class="mx-auto max-w-2xl space-y-4">
    <div v-if="activeTodoGroup.todos.length === 0" class="py-20 text-center text-slate-300">
      <Inbox class="mx-auto mb-4 h-10 w-10 opacity-20" />
      <p>{{ $t("ui.todoPage.emptyDetail") }}</p>
    </div>
    <div
      v-for="(todo, idx) in activeTodoGroup.todos"
      :key="'detail-todo-' + todo.id"
      @click="openEdit(todo)"
      class="glass-effect flex cursor-pointer items-center space-x-4 rounded-2xl border border-slate-100 p-4 shadow-sm transition-all"
      :class="todo.completed ? 'opacity-50' : ''"
    >
      <button
        @click.stop="todo.completed ? undoComplete(todo) : completeTodo(todo)"
        class="h-6 w-6 rounded-lg border-2 transition-all"
        :class="
          todo.completed ? 'border-cyan-500 bg-cyan-500' : 'border-slate-100 hover:border-cyan-500'
        "
      >
        <Check v-if="todo.completed" class="h-4 w-4 text-white" />
      </button>
      <div class="flex-1">
        <p :class="['font-medium text-white', todo.completed ? 'line-through' : '']">
          {{ todo.text }}
        </p>
        <div class="mt-1 flex items-center">
          <Star
            v-for="s in 5"
            :key="'star-detail-' + todo.id + '-' + s"
            :class="[
              'h-3 w-3',
              s <= todo.priority ? 'fill-amber-400 text-amber-400' : 'text-slate-100',
            ]"
          />
        </div>
        <div
          v-if="todo.deadline"
          class="mt-1 flex items-center gap-1 text-[11px] font-bold"
          :class="isOverdue(todo) ? 'text-red-400' : 'text-cyan-500'"
        >
          <CalendarClock class="h-3 w-3" />
          <span>{{ formatDeadline(todo.deadline) }}</span>
          <BellRing v-if="todo.remindBeforeMinutes" class="h-3 w-3" />
        </div>
      </div>
      <button @click.stop="removeItem(idx)" class="p-2 text-slate-200 hover:text-red-400">
        <Trash2 />
      </button>
    </div>
  </div>

  <BaseModal
    :show="showModal"
    :title="modalTitle"
    theme="glass"
    @close="showModal = false"
    @confirm="confirmCreate"
  >
    <!-- 场景1：新建待办分组 -->
    <template v-if="modalMode === 'group'">
      <input
        v-model="formData.groupTitle"
        :placeholder="$t('ui.todoPage.groupNamePlaceholder')"
        :class="glassField"
      />
    </template>

    <!-- 场景2：新建具体任务 -->
    <template v-else>
      <input
        v-model="formData.todoText"
        :placeholder="$t('ui.todoPage.taskContentPlaceholder')"
        :class="glassField"
      />
      <div class="flex items-center space-x-3 rounded-2xl border border-white/10 bg-white/10 p-2">
        <span class="pl-2 text-xs font-bold text-white/50 uppercase">{{
          $t("ui.todoPage.priorityLabel")
        }}</span>
        <button
          v-for="s in 5"
          :key="'prio-' + s"
          @click="formData.priority = s"
          class="transform transition-transform focus:outline-none active:scale-125"
        >
          <Star
            :size="24"
            :class="[s <= formData.priority ? 'fill-amber-400 text-amber-400' : 'text-white/25']"
          />
        </button>
      </div>

      <!-- 截止时间（可选） -->
      <div class="flex items-center gap-2 rounded-2xl border border-white/10 bg-white/10 p-2">
        <CalendarClock class="ml-2 h-4 w-4 shrink-0 text-white/50" />
        <input
          v-model="formData.deadline"
          type="datetime-local"
          class="min-w-0 flex-1 rounded-xl border-none bg-transparent px-2 py-2 text-sm text-white [color-scheme:dark] outline-none"
        />
        <button
          v-if="formData.deadline"
          @click="formData.deadline = ''"
          class="shrink-0 px-2 text-white/50 hover:text-red-400"
        >
          <X :size="16" />
        </button>
      </div>

      <!-- 提前提醒（可选，依赖截止时间） -->
      <div class="flex items-center gap-2 rounded-2xl border border-white/10 bg-white/10 p-2">
        <BellRing
          class="ml-2 h-4 w-4 shrink-0"
          :class="formData.deadline ? 'text-white/50' : 'text-white/25'"
        />
        <select
          v-model="formData.remindBeforeMinutes"
          :disabled="!formData.deadline"
          class="select-dark min-w-0 flex-1 rounded-xl border-none bg-transparent px-2 py-2 text-sm text-white [color-scheme:dark] outline-none disabled:text-white/30"
        >
          <option v-for="opt in reminderOptions" :key="String(opt.value)" :value="opt.value">
            {{ opt.label }}
          </option>
        </select>
      </div>
    </template>
  </BaseModal>
</template>

<script setup lang="ts">
import { ref, computed, reactive, watch, onMounted } from "vue";
import { useUIStore } from "@/stores/modules/ui/ui";
import {
  Trash2,
  Star,
  Folder,
  ChevronRight,
  Zap,
  CheckCircle,
  ChevronDown,
  Inbox,
  Check,
  CalendarClock,
  BellRing,
  X,
} from "lucide-vue-next";
import { getSchedules, saveSchedules } from "@/api/services/schedule";

import BaseModal from "@/components/ui/BaseModal.vue";
import { useI18n } from "vue-i18n";

const { t } = useI18n();
const uiStore = useUIStore();

const showCompleted = ref(false);
const todoInitialized = ref(false);
const preventFirstSave = ref(true);
const selectedTodoGroupId = ref<string | null>(null);

interface TodoItem {
  id: number;
  text: string;
  deadline?: string | null;
  /** 截止前多少分钟提醒；null / 缺省表示不提醒 */
  remindBeforeMinutes?: number | null;
  priority: number;
  completed: boolean;
}

interface TodoGroup {
  title: string;
  description?: string;
  todos: TodoItem[];
}

interface TodoItemWithGroup extends TodoItem {
  groupTitle: string;
  gid: string;
}

const todoGroups = ref<Record<string, TodoGroup>>({});

const loadData = async () => {
  try {
    const data = await getSchedules();
    if (data && data.todoGroups) {
      todoGroups.value = data.todoGroups;
    }
  } catch (e) {
    console.error("Failed to load todos", e);
  } finally {
    todoInitialized.value = true;
  }
};

watch(
  todoGroups,
  async (newVal) => {
    if (!todoInitialized.value) return;
    if (preventFirstSave.value) {
      preventFirstSave.value = false;
      return;
    }

    try {
      await saveSchedules({ todoGroups: newVal });
    } catch (e) {
      console.error("Failed to save todos", e);
    }
  },
  { deep: true },
);

onMounted(() => {
  loadData();
});

const activeTodoGroup = computed(() => {
  if (!selectedTodoGroupId.value) {
    return { todos: [] };
  }
  return todoGroups.value[selectedTodoGroupId.value] || { todos: [] };
});

const globalPendingTodos = computed(() => {
  const list: TodoItemWithGroup[] = [];
  Object.keys(todoGroups.value).forEach((gid) => {
    const group = todoGroups.value[gid];
    if (group) {
      group.todos.forEach((t) => {
        if (!t.completed)
          list.push({
            ...t,
            groupTitle: group.title,
            gid,
          });
      });
    }
  });
  return list.sort((a, b) => b.priority - a.priority);
});

const globalCompletedTodos = computed(() => {
  const list: TodoItemWithGroup[] = [];
  Object.keys(todoGroups.value).forEach((gid) => {
    const group = todoGroups.value[gid];
    if (group) {
      group.todos.forEach((t) => {
        if (t.completed)
          list.push({
            ...t,
            groupTitle: group.title,
            gid,
          });
      });
    }
  });
  return list;
});

// 修改 completeTodo
const completeTodo = (todo: TodoItem | TodoItemWithGroup) => {
  // 兼容两种情况：全局视图传来带有 gid 的 copy 对象，详情视图传来没有 gid 的原始对象
  const targetTodo = resolveOriginal(todo);
  if (targetTodo) {
    targetTodo.completed = true;
  }
};

// 修改 undoComplete
const undoComplete = (todo: TodoItem | TodoItemWithGroup) => {
  const targetTodo = resolveOriginal(todo);
  if (targetTodo) {
    targetTodo.completed = false;
  }
};

const removeItem = (idx: number) => {
  activeTodoGroup.value.todos.splice(idx, 1);
};

const removeTodoGroup = (id: string) => {
  delete todoGroups.value[id];
};

const selectTodoGroup = (id: string) => {
  selectedTodoGroupId.value = id;
  uiStore.scheduleView = "todo_detail";
};
const showModal = ref(false);
/** 弹窗场景。不能只看 scheduleView：分组总览页也能点开待办条目做编辑。 */
const modalMode = ref<"group" | "task">("task");
const editingId = ref<number | null>(null);
const editingGid = ref<string | null>(null);
const formData = reactive({
  groupTitle: "",
  todoText: "",
  priority: 1,
  /** datetime-local 的原生格式：YYYY-MM-DDTHH:mm，空串表示未设 */
  deadline: "",
  remindBeforeMinutes: null as number | null,
});

/** 弹窗内的玻璃态输入框样式，与日程面板的深色毛玻璃风格一致 */
const glassField =
  "w-full rounded-2xl border border-white/10 bg-white/10 px-5 py-4 text-sm text-white transition-all outline-none placeholder:text-white/40 focus:border-brand focus:ring-2 focus:ring-cyan-500/30 [color-scheme:dark]";

const reminderOptions = computed(() => [
  { value: null as number | null, label: t("ui.todoPage.reminderOff") },
  { value: 5, label: t("ui.todoPage.reminderMinutes", { count: 5 }) },
  { value: 15, label: t("ui.todoPage.reminderMinutes", { count: 15 }) },
  { value: 30, label: t("ui.todoPage.reminderMinutes", { count: 30 }) },
  { value: 60, label: t("ui.todoPage.reminderHours", { count: 1 }) },
  { value: 1440, label: t("ui.todoPage.reminderDays", { count: 1 }) },
]);

/** 归一化为 datetime-local 能回显的格式，兼容 AI 写入的自由字符串 */
const toLocalInput = (raw?: string | null): string => {
  if (!raw) return "";
  const normalized = raw.trim().replace(" ", "T").slice(0, 16);
  return /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(normalized) ? normalized : "";
};

const formatDeadline = (raw?: string | null): string => {
  const local = toLocalInput(raw);
  return local ? local.slice(5).replace("T", " ") : "";
};

const isOverdue = (todo: TodoItem): boolean => {
  const local = toLocalInput(todo.deadline);
  if (!local || todo.completed) return false;
  return new Date(local).getTime() < Date.now();
};

/** 按 gid 还原原始条目对象（全局视图传来的是副本，详情视图是原始引用） */
const resolveOriginal = (
  todo: TodoItem | TodoItemWithGroup,
  gid?: string | null,
): TodoItem | undefined => {
  const targetGid = gid ?? (todo as TodoItemWithGroup).gid ?? selectedTodoGroupId.value;
  if (!targetGid) return undefined;
  return todoGroups.value[targetGid]?.todos.find((it) => it.id === todo.id);
};

const modalTitle = computed(() => {
  if (modalMode.value === "group") return t("ui.todoPage.newGroup");
  return editingId.value !== null ? t("ui.todoPage.editTask") : t("ui.todoPage.newTask");
});

const resetForm = () => {
  formData.groupTitle = "";
  formData.todoText = "";
  formData.priority = 1;
  formData.deadline = "";
  formData.remindBeforeMinutes = null;
  editingId.value = null;
  editingGid.value = null;
};

const handleCreate = () => {
  resetForm();
  modalMode.value = uiStore.scheduleView === "todo_groups" ? "group" : "task";
  showModal.value = true;
};

/** 打开编辑弹窗。gid 用于定位条目所属分组。 */
const openEdit = (todo: TodoItem | TodoItemWithGroup, gid?: string) => {
  resetForm();
  modalMode.value = "task";
  editingId.value = todo.id;
  editingGid.value = gid ?? (todo as TodoItemWithGroup).gid ?? selectedTodoGroupId.value;
  formData.todoText = todo.text;
  formData.priority = todo.priority;
  formData.deadline = toLocalInput(todo.deadline);
  formData.remindBeforeMinutes = todo.remindBeforeMinutes ?? null;
  showModal.value = true;
};

const confirmCreate = () => {
  if (modalMode.value === "group") {
    // 新建组
    const newId = "t" + Date.now();
    todoGroups.value[newId] = {
      title: formData.groupTitle,
      todos: [],
    };
  } else {
    const deadline = formData.deadline || null;
    // 没有截止时间的提醒没有意义
    const remindBeforeMinutes = deadline ? formData.remindBeforeMinutes : null;
    if (editingId.value !== null) {
      // 编辑已有任务
      const target = resolveOriginal({ id: editingId.value } as TodoItem, editingGid.value);
      if (target) {
        target.text = formData.todoText;
        target.priority = formData.priority;
        target.deadline = deadline;
        target.remindBeforeMinutes = remindBeforeMinutes;
      }
    } else if (selectedTodoGroupId.value) {
      // 新建任务
      const group = todoGroups.value[selectedTodoGroupId.value];
      if (group) {
        group.todos.push({
          id: Date.now(),
          text: formData.todoText,
          priority: formData.priority,
          completed: false,
          deadline,
          remindBeforeMinutes,
        });
      }
    }
  }
  showModal.value = false;
  resetForm();
};

defineExpose({ handleCreate });
</script>
