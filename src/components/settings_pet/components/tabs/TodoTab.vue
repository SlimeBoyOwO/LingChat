<template>
  <div class="flex h-full w-full flex-col">
    <!-- 主容器：完美保留原有的 Article 结构 -->
    <article class="flex h-full w-full flex-col">
      <!-- 头部区域复刻 -->
      <header
        class="mb-6 flex shrink-0 items-end justify-between border-b-2 pb-2 transition-colors"
        :class="isDarkMode ? 'border-slate-700' : 'border-slate-100'"
      >
        <div>
          <div class="mb-1 flex items-center gap-3">
            <!-- 在详情页时提供返回按钮 -->
            <button
              v-if="activePage === 'todo_detail'"
              @click="activePage = 'todo_groups'"
              class="rounded-md p-1 transition-colors hover:bg-slate-500/20"
              :class="isDarkMode ? 'text-slate-300' : 'text-slate-600'"
              :title="$t('pet.todo.backToGroups')"
            >
              <ArrowLeft class="h-5 w-5" />
            </button>

            <h2
              class="text-xl font-black tracking-wide transition-colors"
              :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'"
            >
              {{
                activePage === "todo_groups" ? $t("pet.todo.groupsTitle") : activeTodoGroup.title
              }}
            </h2>
          </div>
          <p
            class="text-xs font-medium transition-colors"
            :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
          >
            {{
              activePage === "todo_groups" ? $t("pet.todo.groupsDesc") : $t("pet.todo.detailDesc")
            }}
          </p>
        </div>
        <button
          @click="handleCreate"
          class="mr-3 ml-auto flex cursor-pointer items-center gap-1 rounded-lg border px-6 py-3
            text-xs font-bold transition-all"
          :class="
            isDarkMode
              ? 'border-sky-800 bg-sky-900/20 text-sky-400 hover:border-sky-700 hover:bg-sky-900/30'
              : 'border-sky-200 bg-sky-50 text-sky-500 hover:border-sky-300 hover:bg-sky-100'
          "
        >
          <Cross class="h-3.5 w-3.5" />
          <span>{{ $t("pet.todo.create") }}</span>
        </button>
        <span
          class="font-mono text-4xl font-bold uppercase italic transition-colors select-none"
          :class="isDarkMode ? 'text-slate-700' : 'text-sky-100'"
        >
          {{ activePage === "todo_groups" ? "ALL" : "LIST" }}
        </span>
      </header>

      <!-- 滚动内容区 -->
      <div class="flex-1 space-y-8 overflow-y-auto pr-1 pb-4">
        <!-- ================= 视图 1：任务分组总览 ================= -->
        <div v-if="activePage === 'todo_groups'" class="space-y-8">
          <!-- 分组卡片栅格 (完美复刻原闲置状态卡片) -->
          <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
            <div
              v-for="(group, id) in todoGroups"
              :key="'group-' + id"
              @click="selectTodoGroup(id as string)"
              class="group relative flex cursor-pointer flex-col overflow-hidden rounded-xl border
                p-5 shadow-sm transition-colors duration-300"
              :class="
                isDarkMode
                  ? 'border-slate-700 bg-slate-800/50 hover:border-slate-500'
                  : 'border-slate-200 bg-white hover:border-slate-300'
              "
            >
              <!-- 左侧强调线 -->
              <div
                class="absolute top-0 left-0 h-full w-1 transition-colors duration-300"
                :class="
                  isDarkMode
                    ? 'bg-sky-700 group-hover:bg-sky-400'
                    : 'bg-sky-300 group-hover:bg-sky-500'
                "
              ></div>

              <!-- 删除按钮 (悬浮显示) -->
              <button
                @click.stop="removeTodoGroup(id as string)"
                class="absolute top-3 right-3 rounded-md p-1 text-slate-400 opacity-0 transition-all
                  group-hover:opacity-100 hover:bg-red-500/10 hover:text-red-500"
              >
                <Trash2 class="h-4 w-4" />
              </button>

              <div class="flex h-full flex-col gap-1 pl-2">
                <div class="mb-2 flex items-center gap-1.5">
                  <Folder
                    class="h-4 w-4 transition-colors"
                    :class="isDarkMode ? 'text-sky-500' : 'text-sky-600'"
                  />
                  <span
                    class="font-mono text-[10px] font-bold tracking-wider uppercase"
                    :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
                  >
                    TODO GROUP
                  </span>
                </div>
                <h3
                  class="truncate pr-6 text-[15px] font-bold transition-colors"
                  :class="isDarkMode ? 'text-slate-200' : 'text-slate-700'"
                >
                  {{ group.title }}
                </h3>
                <p
                  class="mt-1 mt-auto font-mono text-xs leading-relaxed transition-colors"
                  :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
                >
                  {{ $t("pet.todo.taskCount", { count: group.todos.length }) }}
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
              <Zap class="h-4 w-4 text-sky-500" />
              <h3
                class="font-mono text-xs font-bold tracking-widest uppercase"
                :class="isDarkMode ? 'text-slate-300' : 'text-slate-600'"
              >
                {{ $t("pet.todo.globalPending") }}
              </h3>
            </div>

            <div
              v-if="globalPendingTodos.length === 0"
              class="py-8 text-center text-xs font-medium"
              :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
            >
              {{ $t("pet.todo.noPending") }}
            </div>

            <!-- 任务行卡片化 -->
            <div
              v-for="todo in globalPendingTodos"
              :key="'global-' + todo.id"
              class="relative flex items-center justify-between overflow-hidden rounded-xl border
                p-4 shadow-sm transition-colors duration-300"
              :class="
                isDarkMode ? 'border-slate-700 bg-slate-800/80' : 'border-slate-200 bg-slate-50'
              "
            >
              <!-- 优先级左侧强调线 -->
              <div class="absolute top-0 left-0 h-full w-1 bg-sky-400 opacity-80"></div>

              <div class="flex min-w-0 flex-1 items-center gap-4 pl-2">
                <button
                  @click.stop="completeTodo(todo)"
                  class="group flex h-5 w-5 shrink-0 items-center justify-center rounded-md border-2
                    transition-all"
                  :class="
                    isDarkMode
                      ? 'border-slate-500 hover:border-sky-400'
                      : 'border-slate-300 hover:border-sky-500'
                  "
                >
                  <Check
                    class="h-3 w-3 text-sky-500 opacity-0 transition-opacity
                      group-hover:opacity-100"
                  />
                </button>

                <div class="flex min-w-0 flex-1 flex-col">
                  <div class="mb-1 flex items-center gap-2">
                    <span
                      class="rounded px-1.5 py-0.5 font-mono text-[9px] font-bold"
                      :class="isDarkMode ? 'bg-slate-700 text-sky-400' : 'bg-sky-100 text-sky-600'"
                    >
                      {{ todo.groupTitle }}
                    </span>
                    <div class="flex">
                      <Star
                        v-for="s in 5"
                        :key="'star-global-' + todo.id + '-' + s"
                        class="h-2.5 w-2.5"
                        :class="
                          s <= todo.priority
                            ? 'fill-sky-400 text-sky-400'
                            : isDarkMode
                              ? 'text-slate-600'
                              : 'text-slate-300'
                        "
                      />
                    </div>
                  </div>
                  <p
                    class="truncate text-[13px] font-bold"
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
              class="flex items-center gap-2 px-1 transition-colors"
              :class="
                isDarkMode
                  ? 'text-slate-400 hover:text-sky-400'
                  : 'text-slate-500 hover:text-sky-600'
              "
            >
              <component :is="showCompleted ? ChevronDown : ChevronRight" class="h-4 w-4" />
              <span class="font-mono text-[10px] font-bold tracking-widest uppercase">
                {{ $t("pet.todo.completedHistory", { count: globalCompletedTodos.length }) }}
              </span>
            </button>

            <div v-if="showCompleted" class="space-y-2 pl-2">
              <div
                v-for="todo in globalCompletedTodos"
                :key="'done-' + todo.id"
                class="flex items-center justify-between rounded-lg border p-3 opacity-60
                  transition-opacity hover:opacity-100"
                :class="
                  isDarkMode
                    ? 'border-slate-700/50 bg-slate-800/40'
                    : 'border-slate-200 bg-slate-100/50'
                "
              >
                <div class="flex min-w-0 flex-1 items-center gap-3 pl-1">
                  <CheckCircle class="h-4 w-4 shrink-0 text-emerald-500" />
                  <span
                    class="rounded border px-1 font-mono text-[9px]"
                    :class="
                      isDarkMode
                        ? 'border-slate-600 text-slate-400'
                        : 'border-slate-300 text-slate-500'
                    "
                  >
                    {{ todo.groupTitle }}
                  </span>
                  <p
                    class="truncate text-xs line-through"
                    :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
                  >
                    {{ todo.text }}
                  </p>
                </div>
                <button
                  @click.stop="undoComplete(todo)"
                  class="ml-2 shrink-0 font-mono text-[10px] font-bold transition-colors"
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
            <Inbox class="mb-4 h-12 w-12 opacity-50" />
            <p class="text-sm font-medium">{{ $t("pet.todo.emptyGroup") }}</p>
          </div>

          <div
            v-for="(todo, idx) in activeTodoGroup.todos"
            :key="'detail-todo-' + todo.id"
            class="group relative flex items-center justify-between overflow-hidden rounded-xl
              border p-4 shadow-sm transition-colors duration-300"
            :class="[
              isDarkMode ? 'border-slate-700 bg-slate-800/80' : 'border-slate-200 bg-slate-50',
              todo.completed ? 'opacity-50' : '',
            ]"
          >
            <!-- 任务状态侧边线 -->
            <div
              class="absolute top-0 left-0 h-full w-1"
              :class="todo.completed ? 'bg-emerald-500' : 'bg-sky-400'"
            ></div>

            <div class="flex min-w-0 flex-1 items-center gap-4 pl-2">
              <!-- 勾选框 -->
              <button
                @click.stop="todo.completed ? undoComplete(todo) : completeTodo(todo)"
                class="flex h-5 w-5 shrink-0 items-center justify-center rounded-md border-2
                  transition-all"
                :class="
                  todo.completed
                    ? 'border-emerald-500 bg-emerald-500'
                    : isDarkMode
                      ? 'border-slate-500 hover:border-sky-400'
                      : 'border-slate-300 hover:border-sky-500'
                "
              >
                <Check v-if="todo.completed" class="h-3 w-3 text-white" />
              </button>

              <div class="flex min-w-0 flex-1 flex-col">
                <p
                  class="truncate text-[14px] font-bold transition-all"
                  :class="[
                    isDarkMode ? 'text-slate-200' : 'text-slate-700',
                    todo.completed ? 'text-slate-500 line-through' : '',
                  ]"
                >
                  {{ todo.text }}
                </p>
                <div class="mt-1 flex items-center">
                  <Star
                    v-for="s in 5"
                    :key="'star-detail-' + todo.id + '-' + s"
                    class="h-2.5 w-2.5"
                    :class="
                      s <= todo.priority
                        ? 'fill-sky-400 text-sky-400'
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
              class="ml-2 rounded-md p-2 text-slate-400 opacity-0 transition-all
                group-hover:opacity-100 hover:bg-red-500/10 hover:text-red-500"
            >
              <Trash2 class="h-4 w-4" />
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
            class="font-mono text-xs font-bold tracking-wider"
            :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
            >GROUP NAME</label
          >
          <input
            v-model="formData.groupTitle"
            :placeholder="$t('pet.todo.groupNamePlaceholder')"
            class="w-full rounded-lg border px-3 py-2.5 text-sm transition-all duration-200
              focus:outline-none"
            :class="
              isDarkMode
                ? 'border-slate-700 bg-slate-900/50 text-slate-200 focus:border-sky-500'
                : 'border-slate-200 bg-slate-50 text-slate-700 focus:border-sky-500'
            "
          />
        </div>
      </template>

      <template v-else>
        <div class="space-y-4">
          <div class="space-y-2">
            <label
              class="font-mono text-xs font-bold tracking-wider"
              :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
              >TASK CONTENT</label
            >
            <input
              v-model="formData.todoText"
              :placeholder="$t('pet.todo.taskContentPlaceholder')"
              class="w-full rounded-lg border px-3 py-2.5 text-sm transition-all duration-200
                focus:outline-none"
              :class="
                isDarkMode
                  ? 'border-slate-700 bg-slate-900/50 text-slate-200 focus:border-sky-500'
                  : 'border-slate-200 bg-slate-50 text-slate-700 focus:border-sky-500'
              "
            />
          </div>

          <div
            class="flex items-center gap-3 rounded-lg border p-3"
            :class="
              isDarkMode ? 'border-slate-700 bg-slate-800/50' : 'border-slate-200 bg-slate-50'
            "
          >
            <span
              class="font-mono text-[10px] font-bold tracking-wider uppercase"
              :class="isDarkMode ? 'text-slate-400' : 'text-slate-500'"
              >{{ $t("pet.todo.priority") }}</span
            >
            <div class="ml-auto flex items-center gap-1">
              <button
                v-for="s in 5"
                :key="'prio-' + s"
                @click="formData.priority = s"
                class="transform transition-transform focus:outline-none active:scale-125"
              >
                <Star
                  class="h-5 w-5"
                  :class="
                    s <= formData.priority
                      ? 'fill-sky-400 text-sky-400'
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
  import { useI18n } from "vue-i18n";
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
  const initialized = ref(false);
  const { t } = useI18n();

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
    } finally {
      initialized.value = true;
    }
  };

  watch(
    todoGroups,
    async (newVal) => {
      if (!initialized.value) return;
      try {
        await saveSchedules({ todoGroups: newVal });
      } catch (e) {
        console.error("Failed to save todos", e);
      }
    },
    { deep: true }
  );

  onMounted(() => {
    loadData();
  });

  const activeTodoGroup = computed(() => {
    if (!selectedTodoGroupId.value) {
      return { title: "", todos: [] };
    }
    return todoGroups.value[selectedTodoGroupId.value] || { title: "", todos: [] };
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
      const targetTodo = todoGroups.value[gid].todos.find((t) => t.id === todo.id);
      if (targetTodo) {
        targetTodo.completed = true;
      }
    }
  };

  const undoComplete = (todo: TodoItem | TodoItemWithGroup) => {
    const todoWithGid = todo as TodoItemWithGroup;
    const gid = todoWithGid.gid || selectedTodoGroupId.value;
    if (gid && todoGroups.value[gid]) {
      const targetTodo = todoGroups.value[gid].todos.find((t) => t.id === todo.id);
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
    return activePage.value === "todo_groups" ? t("pet.todo.newGroup") : t("pet.todo.newTask");
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
