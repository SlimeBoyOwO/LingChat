<template>
  <!--
    布局随宽度切换，不靠 JS 平台判断：

    · 窄屏（手机，<768px）：顶部**横向滚动**的标签条 —— 200px 的竖侧栏
      放在 360dp 的屏上只剩 160px 给内容，减掉 p-6 就只剩 112px，滑块没法用。
    · 宽屏（md+，含 1200×800 的桌面窗口）：维持原来的左侧竖栏。

    用 `md:` 而不是 `isMobile()`：判断依据是**可用宽度**，不是设备类型
    （桌面端把窗口拉窄时也该切成横条）。
  -->
  <aside
    class="z-10 flex w-full shrink-0 flex-row overflow-x-auto border-b transition-colors duration-300 md:w-[220px] md:flex-col md:overflow-x-visible md:border-r md:border-b-0"
    :class="isDarkMode ? 'border-slate-700 bg-slate-800/80' : 'border-slate-200 bg-white/80'"
  >
    <!-- 品牌区：窄屏隐藏（横条上没地方放，也没必要） -->
    <div
      class="hidden border-b p-4 transition-colors md:block"
      :class="isDarkMode ? 'border-slate-700/50' : 'border-slate-100'"
    >
      <div class="flex items-center gap-3">
        <div class="relative">
          <div
            class="relative z-10 flex h-10 w-10 items-center justify-center rounded-lg border text-sky-400 transition-colors"
            :class="isDarkMode ? 'border-slate-600 bg-slate-700' : 'border-slate-200 bg-slate-100'"
          >
            <Heart class="h-6 w-6" />
          </div>
          <div
            class="absolute -right-0.5 -bottom-0.5 z-20 h-3 w-3 rounded-full border-2 bg-emerald-400 transition-colors"
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

    <nav
      class="flex flex-1 flex-row overflow-x-auto py-2 md:flex-col md:overflow-x-visible md:overflow-y-auto"
    >
      <button
        v-for="item in tabs"
        :key="item.key"
        type="button"
        @click="$emit('update:activeTab', item.key)"
        class="group relative flex w-auto shrink-0 flex-col items-start overflow-hidden px-4 py-3 transition-all duration-200 md:w-full md:px-5"
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
        <!-- 选中指示条：竖栏在左侧（纵向），横条在底部（横向）——
             两个方向的 transform 不同，所以拆成两个元素各管一档。 -->
        <div
          class="absolute right-0 bottom-0 left-0 h-1 origin-bottom bg-sky-400 transition-transform duration-300 md:hidden"
          :class="activeTab === item.key ? 'scale-x-100' : 'scale-x-0'"
        ></div>
        <div
          class="absolute top-0 bottom-0 left-0 hidden w-1 origin-left bg-sky-400 transition-transform duration-300 md:block"
          :class="activeTab === item.key ? 'scale-x-100' : 'scale-x-0'"
        ></div>

        <div class="relative z-10 flex flex-col items-center gap-1 md:flex-row md:gap-3">
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
          <div class="text-center md:text-left">
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
            <!-- 英文副标题：窄屏隐藏，否则横条会变很高 -->
            <span
              class="mt-0.5 hidden font-mono text-[9px] font-bold tracking-wider transition-colors md:block"
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
