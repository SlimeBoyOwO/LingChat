<template>
  <div v-if="uiStore.scheduleView === 'todo_groups'" class="space-y-8">
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
      <div
        v-for="(group, id) in todoGroups"
        :key="'group-' + id"
        @click="selectTodoGroup(id)"
        class="glass-effect p-5 rounded-2xl border border-slate-100 shadow-sm hover:border-cyan-200 cursor-pointer flex items-center justify-between group transition-all"
      >
        <div class="flex items-center space-x-4">
          <div
            class="w-10 h-10 bg-cyan-500 rounded-xl flex items-center justify-center text-cyan-50 group-hover:bg-cyan-50 group-hover:text-cyan-500 transition-all"
          >
            <Folder />
          </div>
          <div>
            <h4 class="font-bold text-brand">
              {{ group.title }}
            </h4>
            <p class="text-[10px] text-white uppercase font-bold">
              {{ group.todos.length }} 项任务
            </p>
          </div>
        </div>
        <ChevronRight class="text-slate-200 group-hover:text-cyan-500" />
      </div>
    </div>

    <!-- High Priority Global Tasks -->
    <div class="space-y-4">
      <h3 class="text-xs font-black text-slate-50 uppercase tracking-[0.2em] flex items-center">
        <Zap class="w-3 h-3 mr-2 text-amber-400" />
        全局进行中 (按优先级)
      </h3>
      <div
        v-if="globalPendingTodos.length === 0"
        class="text-center py-10 bg-slate-50/50 rounded-3xl border border-dashed border-slate-200 text-slate-400 text-sm"
      >
        暂时没有进行中的任务
      </div>
      <div
        v-for="todo in globalPendingTodos"
        :key="'global-' + todo.id"
        class="glass-effect p-4 rounded-2xl border-l-4 border-l-cyan-500 shadow-sm flex items-center space-x-4"
      >
        <button
          @click.stop="completeTodo(todo)"
          class="w-6 h-6 border-2 border-cyan-100 rounded-lg hover:border-cyan-500 transition-all"
        ></button>
        <div class="flex-1">
          <div class="flex items-center space-x-2">
            <span class="text-[11px] bg-white/80 text-cyan-500 px-1.5 py-0.5 rounded font-bold">{{
              todo.groupTitle
            }}</span>
            <p class="font-bold text-cyan-50">{{ todo.text }}</p>
          </div>
          <div class="flex items-center mt-1">
            <Star
              v-for="s in 5"
              :key="'star-global-' + todo.id + '-' + s"
              :class="[
                'w-3 h-3',
                s <= todo.priority ? 'text-amber-400 fill-amber-400' : 'text-slate-100',
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
        class="flex items-center space-x-2 text-slate-400 hover:text-cyan-600 transition-colors px-1"
      >
        <component :is="showCompleted ? ChevronDown : ChevronRight" class="w-4 h-4" />
        <span class="text-[10px] font-black uppercase tracking-widest"
          >已完成历史 ({{ globalCompletedTodos.length }})</span
        >
      </button>
      <div v-if="showCompleted" class="space-y-2">
        <div
          v-for="todo in globalCompletedTodos"
          :key="'done-' + todo.id"
          class="bg-slate-50/50 p-4 rounded-2xl border border-slate-100 flex items-center space-x-4 opacity-50"
        >
          <CheckCircle class="text-cyan-500 w-5 h-5" />
          <div class="flex-1">
            <div class="flex items-center space-x-2">
              <span
                class="text-[9px] border border-slate-200 text-slate-400 px-1.5 py-0.5 rounded"
                >{{ todo.groupTitle }}</span
              >
              <p class="text-slate-500 line-through text-sm">
                {{ todo.text }}
              </p>
            </div>
          </div>
          <button
            @click.stop="undoComplete(todo)"
            class="text-[10px] text-cyan-600 font-bold hover:underline"
          >
            撤回
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- Todo Detail View -->
  <div v-if="uiStore.scheduleView === 'todo_detail'" class="max-w-2xl mx-auto space-y-4">
    <div v-if="activeTodoGroup.todos.length === 0" class="text-center py-20 text-slate-300">
      <Inbox class="w-10 h-10 mx-auto mb-4 opacity-20" />
      <p>还没有任务，点击右上角新建一个吧</p>
    </div>
    <div
      v-for="(todo, idx) in activeTodoGroup.todos"
      :key="'detail-todo-' + todo.id"
      class="bg-white p-4 rounded-2xl border border-slate-100 shadow-sm flex items-center space-x-4 transition-all"
      :class="todo.completed ? 'opacity-50' : ''"
    >
      <button
        @click.stop="todo.completed ? undoComplete(todo) : completeTodo(todo)"
        class="w-6 h-6 border-2 rounded-lg transition-all"
        :class="
          todo.completed ? 'bg-cyan-500 border-cyan-500' : 'border-slate-100 hover:border-cyan-500'
        "
      >
        <Check v-if="todo.completed" class="text-white w-4 h-4" />
      </button>
      <div class="flex-1">
        <p
          :class="[
            'font-medium',
            todo.completed ? 'line-through text-slate-400' : 'text-slate-700',
          ]"
        >
          {{ todo.text }}
        </p>
        <div class="flex items-center mt-1">
          <Star
            v-for="s in 5"
            :key="'star-detail-' + todo.id + '-' + s"
            :class="[
              'w-3 h-3',
              s <= todo.priority ? 'text-amber-400 fill-amber-400' : 'text-slate-100',
            ]"
          />
        </div>
      </div>
      <button @click.stop="removeItem(idx)" class="text-slate-200 hover:text-red-400 p-2">
        <Trash2 />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useUIStore } from '@/stores/modules/ui/ui'
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
} from 'lucide-vue-next'

const uiStore = useUIStore()

const showCompleted = ref(false)
const selectedTodoGroupId = ref<string | null>(null)

interface TodoItem {
  id: number
  text: string
  deadline?: string
  priority: number
  completed: boolean
}

interface TodoGroup {
  title: string
  description?: string
  todos: TodoItem[]
}

interface TodoItemWithGroup extends TodoItem {
  groupTitle: string
  gid: string
}

const todoGroups = ref<Record<string, TodoGroup>>({
  t1: {
    title: '学校任务',
    todos: [
      {
        id: 101,
        text: '完成计算机视觉报告',
        priority: 5,
        completed: false,
      },
      {
        id: 102,
        text: '数据结构大作业',
        priority: 4,
        completed: false,
      },
    ],
  },
  t2: {
    title: 'LingChat 项目',
    todos: [
      {
        id: 201,
        text: '多角色服装设计',
        priority: 3,
        completed: false,
      },
    ],
  },
})

const activeTodoGroup = computed(() => {
  if (!selectedTodoGroupId.value) {
    return { todos: [] }
  }
  return todoGroups.value[selectedTodoGroupId.value] || { todos: [] }
})

const globalPendingTodos = computed(() => {
  const list: TodoItemWithGroup[] = []
  Object.keys(todoGroups.value).forEach((gid) => {
    const group = todoGroups.value[gid]
    if (group) {
      group.todos.forEach((t) => {
        if (!t.completed)
          list.push({
            ...t,
            groupTitle: group.title,
            gid,
          })
      })
    }
  })
  return list.sort((a, b) => b.priority - a.priority)
})

const globalCompletedTodos = computed(() => {
  const list: TodoItemWithGroup[] = []
  Object.keys(todoGroups.value).forEach((gid) => {
    const group = todoGroups.value[gid]
    if (group) {
      group.todos.forEach((t) => {
        if (t.completed)
          list.push({
            ...t,
            groupTitle: group.title,
            gid,
          })
      })
    }
  })
  return list
})

const completeTodo = (todo: TodoItem) => {
  todo.completed = true
}
const undoComplete = (todo: TodoItem) => {
  todo.completed = false
}

const removeItem = (idx: number) => {
  activeTodoGroup.value.todos.splice(idx, 1)
}

const selectTodoGroup = (id: string) => {
  selectedTodoGroupId.value = id
  uiStore.scheduleView = 'todo_detail'
}
</script>
