<template>
  <header
    class="h-14 shrink-0 border-b flex items-center justify-between px-4 select-none transition-colors duration-300"
    :class="
      isDarkMode
        ? 'bg-slate-800/90 border-slate-700'
        : 'bg-white/90 border-slate-200'
    "
    data-tauri-drag-region
  >
    <div class="flex items-center gap-3 pointer-events-none">
      <div class="w-1.5 h-5 bg-sky-400 rounded-sm skew-x-[-15deg]"></div>
      <div>
        <h1
          class="m-0 text-[15px] font-bold tracking-wide flex items-center gap-1.5 transition-colors"
          :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'"
        >
          Lingchat-Pal
          <span
            class="text-[13px] font-black transition-colors"
            :class="isDarkMode ? 'text-slate-400' : 'text-slate-600'"
            >控制台</span
          >
        </h1>
        <p
          class="m-0 text-[9px] font-mono font-bold tracking-widest uppercase transition-colors"
          :class="isDarkMode ? 'text-sky-400' : 'text-sky-500'"
        >
          Configuration Panel
        </p>
      </div>
    </div>

    <div class="flex items-center gap-1">
      <!-- 主题切换按钮 -->
      <button
        type="button"
        @click="$emit('toggleTheme')"
        class="w-8 h-8 flex items-center justify-center rounded transition-colors pointer-events-auto mr-1"
        :class="
          isDarkMode
            ? 'text-amber-300 hover:bg-slate-700 hover:text-amber-200'
            : 'text-slate-400 hover:bg-slate-100 hover:text-sky-500'
        "
      >
        <Sun v-if="isDarkMode" class="w-4 h-4" />
        <Moon v-else class="w-4 h-4" />
      </button>

      <!-- 分割线 -->
      <div
        class="w-px h-4 mx-1 transition-colors"
        :class="isDarkMode ? 'bg-slate-700' : 'bg-slate-200'"
      ></div>

      <button
        type="button"
        @click="$emit('minimizeWindow')"
        class="w-8 h-8 flex items-center justify-center rounded transition-colors pointer-events-auto"
        :class="
          isDarkMode
            ? 'text-slate-400 hover:bg-slate-700 hover:text-sky-400'
            : 'text-slate-400 hover:bg-slate-100 hover:text-sky-500'
        "
      >
        <Minus class="w-4 h-4" />
      </button>
      <button
        type="button"
        @click="$emit('toggleMaximizeWindow')"
        class="w-8 h-8 flex items-center justify-center rounded transition-colors pointer-events-auto"
        :class="
          isDarkMode
            ? 'text-slate-400 hover:bg-slate-700 hover:text-sky-400'
            : 'text-slate-400 hover:bg-slate-100 hover:text-sky-500'
        "
      >
        <Square class="w-3.5 h-3.5" v-if="!isMaximized" />
        <Copy class="w-3.5 h-3.5" v-else />
      </button>
      <button
        type="button"
        @click="$emit('closeWindow')"
        class="w-8 h-8 flex items-center justify-center rounded transition-colors pointer-events-auto"
        :class="
          isDarkMode
            ? 'text-slate-400 hover:bg-rose-500/20 hover:text-rose-400'
            : 'text-slate-400 hover:bg-red-50 hover:text-rose-400'
        "
      >
        <X class="w-4 h-4" />
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
import { Sun, Moon, Minus, Square, Copy, X } from "lucide-vue-next";

defineProps<{
  isDarkMode: boolean;
  isMaximized: boolean;
}>();

defineEmits<{
  toggleTheme: [];
  minimizeWindow: [];
  toggleMaximizeWindow: [];
  closeWindow: [];
}>();
</script>
