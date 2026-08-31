<template>
  <header
    class="flex h-14 shrink-0 items-center justify-between border-b px-4 transition-colors
      duration-300 select-none"
    :class="isDarkMode ? 'border-slate-700 bg-slate-800/90' : 'border-slate-200 bg-white/90'"
    data-tauri-drag-region
  >
    <div class="pointer-events-none flex items-center gap-3">
      <div class="h-5 w-1.5 skew-x-[-15deg] rounded-sm bg-sky-400"></div>
      <div>
        <h1
          class="m-0 flex items-center gap-1.5 text-[15px] font-bold tracking-wide
            transition-colors"
          :class="isDarkMode ? 'text-slate-100' : 'text-slate-800'"
        >
          Lingchat-Pal
          <span
            class="text-[13px] font-black transition-colors"
            :class="isDarkMode ? 'text-slate-400' : 'text-slate-600'"
            >{{ $t("pet.header.console") }}</span
          >
        </h1>
        <p
          class="m-0 font-mono text-[9px] font-bold tracking-widest uppercase transition-colors"
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
        class="pointer-events-auto mr-1 flex h-8 w-8 items-center justify-center rounded
          transition-colors"
        :class="
          isDarkMode
            ? 'text-amber-300 hover:bg-slate-700 hover:text-amber-200'
            : 'text-slate-400 hover:bg-slate-100 hover:text-sky-500'
        "
      >
        <Sun v-if="isDarkMode" class="h-4 w-4" />
        <Moon v-else class="h-4 w-4" />
      </button>

      <!-- 分割线 -->
      <div
        class="mx-1 h-4 w-px transition-colors"
        :class="isDarkMode ? 'bg-slate-700' : 'bg-slate-200'"
      ></div>

      <button
        type="button"
        @click="$emit('minimizeWindow')"
        class="pointer-events-auto flex h-8 w-8 items-center justify-center rounded
          transition-colors"
        :class="
          isDarkMode
            ? 'text-slate-400 hover:bg-slate-700 hover:text-sky-400'
            : 'text-slate-400 hover:bg-slate-100 hover:text-sky-500'
        "
      >
        <Minus class="h-4 w-4" />
      </button>
      <button
        type="button"
        @click="$emit('toggleMaximizeWindow')"
        class="pointer-events-auto flex h-8 w-8 items-center justify-center rounded
          transition-colors"
        :class="
          isDarkMode
            ? 'text-slate-400 hover:bg-slate-700 hover:text-sky-400'
            : 'text-slate-400 hover:bg-slate-100 hover:text-sky-500'
        "
      >
        <Square class="h-3.5 w-3.5" v-if="!isMaximized" />
        <Copy class="h-3.5 w-3.5" v-else />
      </button>
      <button
        type="button"
        @click="$emit('closeWindow')"
        class="pointer-events-auto flex h-8 w-8 items-center justify-center rounded
          transition-colors"
        :class="
          isDarkMode
            ? 'text-slate-400 hover:bg-rose-500/20 hover:text-rose-400'
            : 'text-slate-400 hover:bg-red-50 hover:text-rose-400'
        "
      >
        <X class="h-4 w-4" />
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
