<template>
  <aside
    class="z-10 flex w-[200px] shrink-0 flex-col border-r transition-colors duration-300
      md:w-[220px]"
    :class="isDarkMode ? 'border-slate-700 bg-slate-800/80' : 'border-slate-200 bg-white/80'"
  >
    <div
      class="border-b p-4 transition-colors"
      :class="isDarkMode ? 'border-slate-700/50' : 'border-slate-100'"
    >
      <div class="flex items-center gap-3">
        <div class="relative">
          <div
            class="relative z-10 flex h-10 w-10 items-center justify-center rounded-lg border
              text-sky-400 transition-colors"
            :class="isDarkMode ? 'border-slate-600 bg-slate-700' : 'border-slate-200 bg-slate-100'"
          >
            <Heart class="h-6 w-6" />
          </div>
          <div
            class="absolute -right-0.5 -bottom-0.5 z-20 h-3 w-3 rounded-full border-2 bg-emerald-400
              transition-colors"
            :class="isDarkMode ? 'border-slate-800' : 'border-white'"
          ></div>
        </div>
        <div>
          <span
            class="mb-0.5 block text-[11px] font-bold tracking-wider uppercase transition-colors"
            :class="isDarkMode ? 'text-slate-500' : 'text-slate-400'"
            >Ling Ling</span
          >
          <strong
            class="block text-[13px] font-black transition-colors"
            :class="isDarkMode ? 'text-slate-200' : 'text-slate-700'"
            >{{ $t("pet.sidebar.title") }}</strong
          >
        </div>
      </div>
    </div>

    <nav class="flex-1 overflow-y-auto py-2">
      <button
        v-for="item in tabs"
        :key="item.key"
        type="button"
        @click="$emit('update:activeTab', item.key)"
        class="group relative flex w-full flex-col items-start overflow-hidden px-5 py-3
          transition-all duration-200"
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
          class="absolute top-0 bottom-0 left-0 w-1 origin-left bg-sky-400 transition-transform
            duration-300"
          :class="activeTab === item.key ? 'scale-x-100' : 'scale-x-0'"
        ></div>

        <div class="relative z-10 flex items-center gap-3">
          <component
            :is="item.icon"
            :class="[
              activeTab === item.key
                ? 'text-sky-500'
                : isDarkMode
                  ? 'text-slate-500 group-hover:text-sky-400'
                  : 'text-slate-400 group-hover:text-sky-400',
              'h-5 w-5 transition-colors',
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
              class="mt-0.5 block font-mono text-[9px] font-bold tracking-wider transition-colors"
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
  import { Heart } from "lucide-vue-next";

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
