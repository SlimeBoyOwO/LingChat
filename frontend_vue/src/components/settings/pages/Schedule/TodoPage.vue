<template>
  <div v-if="currentView === 'todo_groups'" class="space-y-8">
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
      <div
        v-for="(group, id) in todoGroups"
        :key="'group-' + id"
        @click="selectTodoGroup(id)"
        class="bg-white p-5 rounded-2xl border border-slate-100 shadow-sm hover:border-cyan-200 cursor-pointer flex items-center justify-between group transition-all"
      >
        <div class="flex items-center space-x-4">
          <div
            class="w-10 h-10 bg-slate-50 rounded-xl flex items-center justify-center text-slate-400 group-hover:bg-cyan-500 group-hover:text-white transition-all"
          >
            <i data-lucide="folder"></i>
          </div>
          <div>
            <h4 class="font-bold text-slate-800">
              {{ group.title }}
            </h4>
            <p class="text-[10px] text-slate-400 uppercase font-bold">
              {{ group.todos.length }} 项任务
            </p>
          </div>
        </div>
        <i data-lucide="chevron-right" class="text-slate-200 group-hover:text-cyan-500"></i>
      </div>
    </div>

    <!-- High Priority Global Tasks -->
    <div class="space-y-4">
      <h3 class="text-xs font-black text-slate-400 uppercase tracking-[0.2em] flex items-center">
        <i data-lucide="zap" class="w-3 h-3 mr-2 text-amber-400"></i>
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
        class="bg-white/80 p-4 rounded-2xl border-l-4 border-l-cyan-500 shadow-sm flex items-center space-x-4"
      >
        <button
          @click.stop="completeTodo(todo)"
          class="w-6 h-6 border-2 border-cyan-100 rounded-lg hover:border-cyan-500 transition-all"
        ></button>
        <div class="flex-1">
          <div class="flex items-center space-x-2">
            <span class="text-[9px] bg-slate-100 text-slate-500 px-1.5 py-0.5 rounded font-bold">{{
              todo.groupTitle
            }}</span>
            <p class="font-bold text-slate-700">{{ todo.text }}</p>
          </div>
          <div class="flex items-center mt-1">
            <i
              v-for="s in 5"
              :key="'star-global-' + todo.id + '-' + s"
              data-lucide="star"
              :class="[
                'w-3 h-3',
                s <= todo.priority ? 'text-amber-400 fill-amber-400' : 'text-slate-100',
              ]"
            ></i>
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
        <i :data-lucide="showCompleted ? 'chevron-down' : 'chevron-right'" class="w-4 h-4"></i>
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
          <i data-lucide="check-circle" class="text-cyan-500 w-5 h-5"></i>
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
  <div v-if="currentView === 'todo_detail'" class="max-w-2xl mx-auto space-y-4">
    <div v-if="activeTodoGroup.todos.length === 0" class="text-center py-20 text-slate-300">
      <i data-lucide="inbox" class="w-10 h-10 mx-auto mb-4 opacity-20"></i>
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
        <i v-if="todo.completed" data-lucide="check" class="text-white w-4 h-4"></i>
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
          <i
            v-for="s in 5"
            :key="'star-detail-' + todo.id + '-' + s"
            data-lucide="star"
            :class="[
              'w-3 h-3',
              s <= todo.priority ? 'text-amber-400 fill-amber-400' : 'text-slate-100',
            ]"
          ></i>
        </div>
      </div>
      <button @click.stop="removeItem('todo', idx)" class="text-slate-200 hover:text-red-400 p-2">
        <i data-lucide="trash-2" class="w-4 h-4"></i>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

const showCompleted = ref(false)

const todoGroups = ref({
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

const activeTodoGroup = computed(() => todoGroups.value[selectedTodoGroupId.value] || { todos: [] })

const globalPendingTodos = computed(() => {
  const list = []
  Object.keys(todoGroups.value).forEach((gid) => {
    todoGroups.value[gid].todos.forEach((t) => {
      if (!t.completed)
        list.push({
          ...t,
          groupTitle: todoGroups.value[gid].title,
          gid,
        })
    })
  })
  return list.sort((a, b) => b.priority - a.priority)
})

const globalCompletedTodos = computed(() => {
  const list = []
  Object.keys(todoGroups.value).forEach((gid) => {
    todoGroups.value[gid].todos.forEach((t) => {
      if (t.completed)
        list.push({
          ...t,
          groupTitle: todoGroups.value[gid].title,
          gid,
        })
    })
  })
  return list
})

// Vue 3 中 props 默认名称是 modelValue
const props = defineProps({
  currentView: String, // 自定义名称
})

const selectedTodoGroupId = ref(null)

const completeTodo = (todo) => {
  todo.completed = true
}
const undoComplete = (todo) => {
  todo.completed = false
}

const removeItem = (idx) => {
  activeTodoGroup.value.todos.splice(idx, 1)
}

const selectTodoGroup = (id) => {
  selectedTodoGroupId.value = id
  currentView.value = 'todo_detail'
}
</script>
