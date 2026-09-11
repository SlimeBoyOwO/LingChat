<template>
  <div v-if="uiStore.scheduleView === 'todo_groups'" class="space-y-8">
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
      <div
        v-for="(group, id) in todoGroups"
        :key="'group-' + id"
        @click="selectTodoGroup(id)"
        class="glass-effect group relative flex cursor-pointer items-center justify-between
          rounded-2xl border border-slate-100 p-5 shadow-sm transition-all hover:border-cyan-200"
      >
        <button
          @click.stop="removeTodoGroup(id)"
          class="absolute top-1 left-1 z-10 p-1 text-slate-200 hover:text-red-400"
        >
          <Trash2 :size="18" />
        </button>
        <div class="ml-2 flex items-center space-x-4">
          <div
            class="flex h-10 w-10 items-center justify-center rounded-xl bg-cyan-500 text-cyan-50
              transition-all group-hover:bg-cyan-50 group-hover:text-cyan-500"
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
        class="text-brand font-blod rounded-3xl border border-dashed border-slate-200 py-10
          text-center text-xl"
      >
        {{ $t("ui.todoPage.noPending") }}
      </div>
      <div
        v-for="todo in globalPendingTodos"
        :key="'global-' + todo.id"
        class="glass-effect flex items-center space-x-4 rounded-2xl border-l-4 border-l-cyan-500 p-4
          shadow-sm"
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
        </div>
      </div>
    </div>

    <!-- Global Completed History -->
    <div v-if="globalCompletedTodos.length > 0" class="space-y-3">
      <button
        @click="showCompleted = !showCompleted"
        class="flex items-center space-x-2 px-1 text-slate-400 transition-colors
          hover:text-cyan-600"
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
          class="flex items-center space-x-4 rounded-2xl border border-slate-100 bg-slate-50/50 p-4
            opacity-50"
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
      class="glass-effect flex items-center space-x-4 rounded-2xl border border-slate-100 p-4
        shadow-sm transition-all"
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
      </div>
      <button @click.stop="removeItem(idx)" class="p-2 text-slate-200 hover:text-red-400">
        <Trash2 />
      </button>
    </div>
  </div>

  <BaseModal
    :show="showModal"
    :title="modalTitle"
    @close="showModal = false"
    @confirm="confirmCreate"
  >
    <!-- 场景1：新建待办分组 -->
    <template v-if="uiStore.scheduleView === 'todo_groups'">
      <input
        v-model="formData.groupTitle"
        :placeholder="$t('ui.todoPage.groupNamePlaceholder')"
        class="w-full rounded-2xl border-none bg-slate-100 px-5 py-4 transition-all outline-none
          focus:ring-2 focus:ring-cyan-500/50"
      />
    </template>

    <!-- 场景2：新建具体任务 -->
    <template v-else>
      <input
        v-model="formData.todoText"
        :placeholder="$t('ui.todoPage.taskContentPlaceholder')"
        class="w-full rounded-2xl border-none bg-slate-100 px-5 py-4 transition-all outline-none
          focus:ring-2 focus:ring-cyan-500/50"
      />
      <div class="flex items-center space-x-3 rounded-2xl bg-slate-50 p-2">
        <span class="pl-2 text-xs font-bold text-slate-400 uppercase">{{
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
            :class="[s <= formData.priority ? 'fill-amber-400 text-amber-400' : 'text-slate-200']"
          />
        </button>
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
    { deep: true }
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
    console.log("完成代办");
    // 兼容两种情况：全局视图传来带有 gid 的 copy 对象，详情视图传来没有 gid 的原始对象
    const todoWithGid = todo as TodoItemWithGroup;
    const gid = todoWithGid.gid || selectedTodoGroupId.value;

    // 在原始数据源中找到真正的那个 todo 对象并修改它
    if (gid && todoGroups.value[gid]) {
      const targetTodo = todoGroups.value[gid].todos.find((t) => t.id === todo.id);
      if (targetTodo) {
        targetTodo.completed = true;
      }
    }
  };

  // 修改 undoComplete
  const undoComplete = (todo: TodoItem | TodoItemWithGroup) => {
    const todoWithGid = todo as TodoItemWithGroup;
    const gid = todoWithGid.gid || selectedTodoGroupId.value;

    // 在原始数据源中找到真正的那个 todo 对象并修改它
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
  };

  const selectTodoGroup = (id: string) => {
    selectedTodoGroupId.value = id;
    uiStore.scheduleView = "todo_detail";
  };
  const showModal = ref(false);
  const formData = reactive({
    groupTitle: "",
    todoText: "",
    priority: 1,
  });

  const modalTitle = computed(() => {
    return uiStore.scheduleView === "todo_groups"
      ? t("ui.todoPage.newGroup")
      : t("ui.todoPage.newTask");
  });

  const handleCreate = () => {
    formData.groupTitle = "";
    formData.todoText = "";
    formData.priority = 1;
    showModal.value = true;
  };

  const confirmCreate = () => {
    if (uiStore.scheduleView === "todo_groups") {
      // 新建组
      const newId = "t" + Date.now();
      todoGroups.value[newId] = {
        title: formData.groupTitle,
        todos: [],
      };
    } else {
      // 新建任务
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
