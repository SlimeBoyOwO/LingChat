<template>
  <aside
    class="w-[200px] md:w-[220px] shrink-0 border-r flex flex-col z-10 transition-colors duration-300"
    :class="
      isDarkMode
        ? 'bg-slate-800/80 border-slate-700'
        : 'bg-white/80 border-slate-200'
    "
  >
    <div
      class="p-4 border-b transition-colors"
      :class="isDarkMode ? 'border-slate-700/50' : 'border-slate-100'"
    >
      <div class="flex items-center gap-3">
        <div class="relative">
          <div
            class="w-10 h-10 rounded-lg border flex items-center justify-center relative z-10 text-sky-400 transition-colors"
            :class="
              isDarkMode
                ? 'bg-slate-700 border-slate-600'
                : 'bg-slate-100 border-slate-200'
            "
          >
            <Heart class="w-6 h-6" />
          </div>
          <div
            class="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-emerald-400 border-2 rounded-full z-20 transition-colors"
            :class="isDarkMode ? 'border-slate-800' : 'border-white'"
          ></div>
        </div>
        <div>
          <span
            class="block text-[11px] font-bold mb-0.5 uppercase tracking-wider transition-colors"
            :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
            >Ling Ling</span
          >
          <strong
            class="block text-[13px] font-black transition-colors"
            :class="isDarkMode ? 'text-slate-200' : 'text-slate-700'"
            >小灵设置站</strong
          >
        </div>
      </div>
    </div>

    <nav class="flex-1 py-2 overflow-y-auto">
      <button
        v-for="item in tabs"
        :key="item.key"
        type="button"
        @click="$emit('update:activeTab', item.key)"
        class="w-full relative px-5 py-3 flex flex-col items-start transition-all duration-200 group overflow-hidden"
        :class="[
          activeTab === item.key
            ? isDarkMode
              ? 'bg-sky-500/10'
              : 'bg-sky-50/50'
            : isDarkMode
              ? 'hover:bg-slate-700/50'
              : 'hover:bg-slate-50',
        ]"
      >
        <div
          class="absolute left-0 top-0 bottom-0 w-1 bg-sky-400 transition-transform duration-300 origin-left"
          :class="activeTab === item.key ? 'scale-x-100' : 'scale-x-0'"
        ></div>

        <div class="flex items-center gap-3 relative z-10">
          <component
            :is="item.icon"
            :class="[
              activeTab === item.key
                ? 'text-sky-500'
                : isDarkMode
                  ? 'text-slate-500 group-hover:text-sky-400'
                  : 'text-slate-400 group-hover:text-sky-400',
              'w-5 h-5 transition-colors',
            ]"
          />
          <div class="text-left">
            <span
              class="block text-[14px] font-bold transition-colors"
              :class="
                activeTab === item.key
                  ? isDarkMode
                    ? 'text-sky-400'
                    : 'text-sky-600'
                  : isDarkMode
                    ? 'text-slate-300'
                    : 'text-slate-600'
              "
            >
              {{ item.label }}
            </span>
            <span
              class="block text-[9px] font-mono font-bold tracking-wider mt-0.5 transition-colors"
              :class="
                activeTab === item.key
                  ? 'text-sky-400/70'
                  : isDarkMode
                    ? 'text-slate-500'
                    : 'text-slate-400/60'
              "
            >
              {{ item.en }}
            </span>
          </div>
        </div>
      </button>
    </nav>
  </aside>
</template>

<script setup lang="ts">
import { Heart, User } from "lucide-vue-next";

type TabItem = {
  key: "pet" | "interaction" | "window" | "todo";
  label: string;
  icon: any;
  en: string;
};

defineProps<{
  isDarkMode: boolean;
  activeTab: "pet" | "interaction" | "window" | "todo";
  tabs: TabItem[];
}>();

defineEmits<{
  "update:activeTab": [value: "pet" | "interaction" | "window" | "todo"];
}>();
</script>
