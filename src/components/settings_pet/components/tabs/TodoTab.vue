<template>
  <div class="w-full h-full flex flex-col">
    <!-- 主容器：完美保留原有的 Article 结构 -->
    <article class="w-full h-full flex flex-col">
      <!-- 头部区域复刻 -->
      <header
        class="mb-6 flex items-end justify-between border-b-2 pb-2 transition-colors shrink-0"
        :class="isDarkMode ? 'border-slate-700' : 'border-slate-100'"
      >
        <div>
          <div class="flex items-center gap-3 mb-1">
            <!-- 在详情页时提供返回按钮 -->
            <button
              v-if="activePage === 'todo_detail'"
              @click="activePage = 'todo_groups'"
              class="p-1 rounded-md transition-colors hover:bg-slate-500/20"
              :class="isDarkMode ? 'text-slate-300' : 'text-slate-600'"
              title="返回分组列表"
            >
              <ArrowLeft class="w-5 h-5" />
            </button>

            <h2
              class="text-xl font-black tracking-wide transition-colors"
              :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'"
            >
              {{
                activePage === "todo_groups"
                  ? "待办任务总览"
                  : activeTodoGroup.title
              }}
            </h2>
          </div>
          <p
            class="text-xs font-medium transition-colors"
            :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
          >
            {{
              activePage === "todo_groups"
                ? "管理您的任务分组与全局进度"
                : "管理当前分组下的所有任务"
            }}
          </p>
        </div>
        <button
          @click="handleCreate"
          class="px-6 py-3 text-xs font-bold rounded-lg transition-all flex items-center gap-1 border cursor-pointer ml-auto mr-3"
          :class="
            isDarkMode
              ? 'bg-sky-900/20 text-sky-400 border-sky-800 hover:bg-sky-900/30 hover:border-sky-700'
              : 'bg-sky-50 text-sky-500 border-sky-200 hover:bg-sky-100 hover:border-sky-300'
          "
        >
          <Cross class="w-3.5 h-3.5" />
          <span>新建</span>
        </button>
        <span
          class="text-4xl font-bold italic select-none font-mono transition-colors uppercase"
          :class="isDarkMode ? 'text-slate-700' : 'text-sky-100'"
        >
          {{ activePage === "todo_groups" ? "ALL" : "LIST" }}
        </span>
      </header>

      <!-- 滚动内容区 -->
      <div class="flex-1 overflow-y-auto pb-4 pr-1 space-y-8">
        <!-- ================= 视图 1：任务分组总览 ================= -->
        <div v-if="activePage === 'todo_groups'" class="space-y-8">
          <!-- 分组卡片栅格 (完美复刻原闲置状态卡片) -->
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div
              v-for="(group, id) in todoGroups"
              :key="'group-' + id"
              @click="selectTodoGroup(id as string)"
              class="p-5 rounded-xl border shadow-sm relative overflow-hidden group cursor-pointer transition-colors duration-300 flex flex-col"
              :class="
                isDarkMode
                  ? 'bg-slate-800/50 border-slate-700 hover:border-slate-500'
                  : 'bg-white border-slate-200 hover:border-slate-300'
              "
            >
              <!-- 左侧强调线 -->
              <div
                class="absolute top-0 left-0 w-1 h-full transition-colors duration-300"
                :class="
                  isDarkMode
                    ? 'bg-sky-700 group-hover:bg-sky-400'
                    : 'bg-sky-300 group-hover:bg-sky-500'
                "
              ></div>

              <!-- 删除按钮 (悬浮显示) -->
              <button
                @click.stop="removeTodoGroup(id as string)"
                class="absolute top-3 right-3 text-slate-400 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-all p-1 rounded-md hover:bg-red-500/10"
              >
                <Trash2 class="w-4 h-4" />
              </button>

              <div class="flex flex-col gap-1 pl-2 h-full">
                <div class="flex items-center gap-1.5 mb-2">
                  <Folder
                    class="w-4 h-4 transition-colors"
                    :class="isDarkMode ? 'text-sky-500' : 'text-sky-600'"
                  />
                  <span
                    class="text-[10px] font-mono font-bold tracking-wider uppercase"
                    :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
                  >
                    TODO GROUP
                  </span>
                </div>
                <h3
                  class="font-bold text-[15px] transition-colors truncate pr-6"
                  :class="isDarkMode ? 'text-slate-200' : 'text-slate-700'"
                >
                  {{ group.title }}
                </h3>
                <p
                  class="text-xs mt-1 leading-relaxed transition-colors mt-auto font-mono"
                  :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
                >
                  {{ group.todos.length }} 项任务
                </p>
              </div>
            </div>
          </div>

          <!-- 全局进行中任务列表 -->
          <div class="space-y-3">
            <div
              class="flex items-center gap-2 border-b pb-2"
              :class="isDarkMode ? 'border-slate-700' : 'border-slate-200'"
            >
              <Zap class="w-4 h-4 text-sky-500" />
              <h3
                class="text-xs font-bold font-mono tracking-widest uppercase"
                :class="isDarkMode ? 'text-slate-300' : 'text-slate-600'"
              >
                全局进行中
              </h3>
            </div>

            <div
              v-if="globalPendingTodos.length === 0"
              class="text-center py-8 text-xs font-medium"
              :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
            >
              暂时没有进行中的任务，很闲哦！
            </div>

            <!-- 任务行卡片化 -->
            <div
              v-for="todo in globalPendingTodos"
              :key="'global-' + todo.id"
              class="p-4 rounded-xl border shadow-sm relative overflow-hidden flex items-center justify-between transition-colors duration-300"
              :class="
                isDarkMode
                  ? 'bg-slate-800/80 border-slate-700'
                  : 'bg-slate-50 border-slate-200'
              "
            >
              <!-- 优先级左侧强调线 -->
              <div
                class="absolute top-0 left-0 w-1 h-full bg-sky-400 opacity-80"
              ></div>

              <div class="flex items-center gap-4 pl-2 flex-1 min-w-0">
                <button
                  @click.stop="completeTodo(todo)"
                  class="w-5 h-5 rounded-md border-2 shrink-0 transition-all flex items-center justify-center group"
                  :class="
                    isDarkMode
                      ? 'border-slate-500 hover:border-sky-400'
                      : 'border-slate-300 hover:border-sky-500'
                  "
                >
                  <Check
                    class="w-3 h-3 opacity-0 group-hover:opacity-100 text-sky-500 transition-opacity"
                  />
                </button>

                <div class="flex flex-col min-w-0 flex-1">
                  <div class="flex items-center gap-2 mb-1">
                    <span
                      class="text-[9px] font-mono font-bold px-1.5 py-0.5 rounded"
                      :class="
                        isDarkMode
                          ? 'bg-slate-700 text-sky-400'
                          : 'bg-sky-100 text-sky-600'
                      "
                    >
                      {{ todo.groupTitle }}
                    </span>
                    <div class="flex">
                      <Star
                        v-for="s in 5"
                        :key="'star-global-' + todo.id + '-' + s"
                        class="w-2.5 h-2.5"
                        :class="
                          s <= todo.priority
                            ? 'text-sky-400 fill-sky-400'
                            : isDarkMode
                              ? 'text-slate-600'
                              : 'text-slate-300'
                        "
                      />
                    </div>
                  </div>
                  <p
                    class="text-[13px] font-bold truncate"
                    :class="isDarkMode ? 'text-slate-200' : 'text-slate-700'"
                  >
                    {{ todo.text }}
                  </p>
                </div>
              </div>
            </div>
          </div>

          <!-- 已完成历史 -->
          <div v-if="globalCompletedTodos.length > 0" class="space-y-3 pt-4">
            <button
              @click="showCompleted = !showCompleted"
              class="flex items-center gap-2 transition-colors px-1"
              :class="
                isDarkMode
                  ? 'text-slate-400 hover:text-sky-400'
                  : 'text-slate-500 hover:text-sky-600'
              "
            >
              <component
                :is="showCompleted ? ChevronDown : ChevronRight"
                class="w-4 h-4"
              />
              <span
                class="text-[10px] font-bold font-mono uppercase tracking-widest"
              >
                已完成历史 ({{ globalCompletedTodos.length }})
              </span>
            </button>

            <div v-if="showCompleted" class="space-y-2 pl-2">
              <div
                v-for="todo in globalCompletedTodos"
                :key="'done-' + todo.id"
                class="p-3 rounded-lg border flex items-center justify-between opacity-60 transition-opacity hover:opacity-100"
                :class="
                  isDarkMode
                    ? 'bg-slate-800/40 border-slate-700/50'
                    : 'bg-slate-100/50 border-slate-200'
                "
              >
                <div class="flex items-center gap-3 flex-1 min-w-0 pl-1">
                  <CheckCircle class="w-4 h-4 text-emerald-500 shrink-0" />
                  <span
                    class="text-[9px] font-mono border rounded px-1"
                    :class="
                      isDarkMode
                        ? 'border-slate-600 text-slate-400'
                        : 'border-slate-300 text-slate-500'
                    "
                  >
                    {{ todo.groupTitle }}
                  </span>
                  <p
                    class="text-xs line-through truncate"
                    :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
                  >
                    {{ todo.text }}
                  </p>
                </div>
                <button
                  @click.stop="undoComplete(todo)"
                  class="text-[10px] font-bold font-mono transition-colors ml-2 shrink-0"
                  :class="
                    isDarkMode
                      ? 'text-sky-400 hover:text-sky-300'
                      : 'text-sky-600 hover:text-sky-500'
                  "
                >
                  UNDO
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- ================= 视图 2：特定任务组详情 ================= -->
        <div v-if="activePage === 'todo_detail'" class="space-y-3">
          <div
            v-if="activeTodoGroup.todos.length === 0"
            class="flex flex-col items-center justify-center py-20 text-center"
            :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
          >
            <Inbox class="w-12 h-12 mb-4 opacity-50" />
            <p class="text-sm font-medium">还没有任务，点击右上角新建一个吧</p>
          </div>

          <div
            v-for="(todo, idx) in activeTodoGroup.todos"
            :key="'detail-todo-' + todo.id"
            class="p-4 rounded-xl border shadow-sm relative overflow-hidden flex items-center justify-between transition-colors duration-300 group"
            :class="[
              isDarkMode
                ? 'bg-slate-800/80 border-slate-700'
                : 'bg-slate-50 border-slate-200',
              todo.completed ? 'opacity-50' : '',
            ]"
          >
            <!-- 任务状态侧边线 -->
            <div
              class="absolute top-0 left-0 w-1 h-full"
              :class="todo.completed ? 'bg-emerald-500' : 'bg-sky-400'"
            ></div>

            <div class="flex items-center gap-4 pl-2 flex-1 min-w-0">
              <!-- 勾选框 -->
              <button
                @click.stop="
                  todo.completed ? undoComplete(todo) : completeTodo(todo)
                "
                class="w-5 h-5 rounded-md border-2 shrink-0 transition-all flex items-center justify-center"
                :class="
                  todo.completed
                    ? 'bg-emerald-500 border-emerald-500'
                    : isDarkMode
                      ? 'border-slate-500 hover:border-sky-400'
                      : 'border-slate-300 hover:border-sky-500'
                "
              >
                <Check v-if="todo.completed" class="w-3 h-3 text-white" />
              </button>

              <div class="flex flex-col min-w-0 flex-1">
                <p
                  class="text-[14px] font-bold truncate transition-all"
                  :class="[
                    isDarkMode ? 'text-slate-200' : 'text-slate-700',
                    todo.completed ? 'line-through text-slate-500' : '',
                  ]"
                >
                  {{ todo.text }}
                </p>
                <div class="flex items-center mt-1">
                  <Star
                    v-for="s in 5"
                    :key="'star-detail-' + todo.id + '-' + s"
                    class="w-2.5 h-2.5"
                    :class="
                      s <= todo.priority
                        ? 'text-sky-400 fill-sky-400'
                        : isDarkMode
                          ? 'text-slate-600'
                          : 'text-slate-300'
                    "
                  />
                </div>
              </div>
            </div>

            <!-- 删除按钮 -->
            <button
              @click.stop="removeItem(idx)"
              class="text-slate-400 hover:text-red-500 opacity-0 group-hover:opacity-100 transition-all p-2 rounded-md hover:bg-red-500/10 ml-2"
            >
              <Trash2 class="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
    </article>

    <!-- BaseModal 使用同样的输入框样式体系 -->
    <BaseModal
      :show="showModal"
      :title="modalTitle"
      @close="showModal = false"
      @confirm="confirmCreate"
    >
      <template v-if="activePage === 'todo_groups'">
        <div class="space-y-2">
          <label
            class="text-xs font-bold font-mono tracking-wider"
            :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
            >GROUP NAME</label
          >
          <input
            v-model="formData.groupTitle"
            placeholder="项目名称 (例如: 学校任务)"
            class="w-full px-3 py-2.5 border rounded-lg text-sm transition-all duration-200 focus:outline-none"
            :class="
              isDarkMode
                ? 'bg-slate-900/50 border-slate-700 text-slate-200 focus:border-sky-500'
                : 'bg-slate-50 border-slate-200 text-slate-700 focus:border-sky-500'
            "
          />
        </div>
      </template>

      <template v-else>
        <div class="space-y-4">
          <div class="space-y-2">
            <label
              class="text-xs font-bold font-mono tracking-wider"
              :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
              >TASK CONTENT</label
            >
            <input
              v-model="formData.todoText"
              placeholder="任务内容"
              class="w-full px-3 py-2.5 border rounded-lg text-sm transition-all duration-200 focus:outline-none"
              :class="
                isDarkMode
                  ? 'bg-slate-900/50 border-slate-700 text-slate-200 focus:border-sky-500'
                  : 'bg-slate-50 border-slate-200 text-slate-700 focus:border-sky-500'
              "
            />
          </div>

          <div
            class="flex items-center gap-3 p-3 rounded-lg border"
            :class="
              isDarkMode
                ? 'bg-slate-800/50 border-slate-700'
                : 'bg-slate-50 border-slate-200'
            "
          >
            <span
              class="text-[10px] font-bold font-mono uppercase tracking-wider"
              :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
              >优先级 PRIORITY</span
            >
            <div class="flex items-center ml-auto gap-1">
              <button
                v-for="s in 5"
                :key="'prio-' + s"
                @click="formData.priority = s"
                class="focus:outline-none transform active:scale-125 transition-transform"
              >
                <Star
                  class="w-5 h-5"
                  :class="
                    s <= formData.priority
                      ? 'text-sky-400 fill-sky-400'
                      : isDarkMode
                        ? 'text-slate-600'
                        : 'text-slate-300'
                  "
                />
              </button>
            </div>
          </div>
        </div>
      </template>
    </BaseModal>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, reactive, watch, onMounted } from "vue";
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
  ArrowLeft,
  Cross,
} from "lucide-vue-next";
import { getSchedules, saveSchedules } from "../../../../api/services/schedule";
import BaseModal from "@/components/ui/BaseModal.vue";

// 必须接收暗色模式状态
defineProps<{
  isDarkMode: boolean;
}>();

const showCompleted = ref(false);
const selectedTodoGroupId = ref<string | null>(null);

const activePage = ref<"todo_detail" | "todo_groups">("todo_groups");

interface TodoItem {
  id: number;
  text: string;
  deadline?: string;
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
  }
};

watch(
  todoGroups,
  async (newVal) => {
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
    return { title: "", todos: [] };
  }
  return (
    todoGroups.value[selectedTodoGroupId.value] || { title: "", todos: [] }
  );
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

const completeTodo = (todo: TodoItem | TodoItemWithGroup) => {
  const todoWithGid = todo as TodoItemWithGroup;
  const gid = todoWithGid.gid || selectedTodoGroupId.value;
  if (gid && todoGroups.value[gid]) {
    const targetTodo = todoGroups.value[gid].todos.find(
      (t) => t.id === todo.id,
    );
    if (targetTodo) {
      targetTodo.completed = true;
    }
  }
};

const undoComplete = (todo: TodoItem | TodoItemWithGroup) => {
  const todoWithGid = todo as TodoItemWithGroup;
  const gid = todoWithGid.gid || selectedTodoGroupId.value;
  if (gid && todoGroups.value[gid]) {
    const targetTodo = todoGroups.value[gid].todos.find(
      (t) => t.id === todo.id,
    );
    if (targetTodo) {
      targetTodo.completed = false;
    }
  }
};

const removeItem = (idx: number) => {
  activeTodoGroup.value.todos.splice(idx, 1);
};

const removeTodoGroup = (id: string) => {
  delete todoGroups.value[id];
  // 如果删除的是当前选中的组，返回总览
  if (selectedTodoGroupId.value === id) {
    activePage.value = "todo_groups";
    selectedTodoGroupId.value = null;
  }
};

const selectTodoGroup = (id: string) => {
  selectedTodoGroupId.value = id;
  activePage.value = "todo_detail";
};

const showModal = ref(false);
const formData = reactive({
  groupTitle: "",
  todoText: "",
  priority: 1,
});

const modalTitle = computed(() => {
  return activePage.value === "todo_groups" ? "新建任务组" : "新建待办任务";
});

const handleCreate = () => {
  formData.groupTitle = "";
  formData.todoText = "";
  formData.priority = 1;
  showModal.value = true;
};

const confirmCreate = () => {
  if (activePage.value === "todo_groups") {
    if (!formData.groupTitle.trim()) return;
    const newId = "t" + Date.now();
    todoGroups.value[newId] = {
      title: formData.groupTitle,
      todos: [],
    };
  } else {
    if (!formData.todoText.trim()) return;
    if (selectedTodoGroupId.value) {
      const group = todoGroups.value[selectedTodoGroupId.value];
      if (group) {
        group.todos.push({
          id: Date.now(),
          text: formData.todoText,
          priority: formData.priority,
          completed: false,
        });
      }
    }
  }
  showModal.value = false;
};

defineExpose({ handleCreate });
</script>
