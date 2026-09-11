<template>
  <MenuPage>
    <MenuItem :title="$t('settings.achievement.title')">
      <template #header>
        <Award :size="20" />
      </template>
      <div class="grid grid-cols-1 gap-5 md:grid-cols-2 xl:grid-cols-3">
        <div
          v-for="achievement in achievementsList"
          :key="achievement.id"
          class="group relative flex items-center overflow-hidden rounded-xl border p-4
            transition-all duration-300"
          :class="getCardClass(achievement)"
        >
          <!-- Rare Effect Background -->
          <div
            v-if="achievement.type === 'rare' && achievement.unlocked"
            class="absolute inset-0 animate-pulse bg-yellow-400/10 blur-2xl"
          ></div>

          <!-- Icon -->
          <div class="relative z-10 mr-4 shrink-0">
            <div
              class="flex h-16 w-16 items-center justify-center rounded-full border-2 shadow-md
                transition-all duration-300"
              :class="getIconClass(achievement)"
            >
              <img
                v-if="achievement.imgUrl"
                :src="achievement.imgUrl"
                class="h-10 w-10 object-contain transition-all duration-300"
                :class="{ 'opacity-40': !achievement.unlocked }"
              />
              <Icon v-else icon="achievement" :size="32" :class="getIconSvgClass(achievement)" />
            </div>
          </div>

          <!-- Info -->
          <div class="z-10 min-w-0 flex-1">
            <div class="mb-1.5 flex items-center justify-between">
              <h3
                class="truncate text-base font-bold tracking-wide"
                :class="achievement.unlocked ? 'text-white text-shadow-sm' : 'text-white/90'"
              >
                {{ achievementTitle(achievement) }}
              </h3>
              <span
                v-if="achievement.unlocked"
                class="rounded-full border px-2 py-0.5 text-[10px] font-medium shadow-sm
                  backdrop-blur-md"
                :class="getBadgeClass(achievement)"
              >
                {{
                  achievement.type === "rare"
                    ? $t("settings.achievement.rare")
                    : $t("settings.achievement.normal")
                }}
              </span>
            </div>

            <p
              class="mb-2 text-xs leading-4 whitespace-pre-line transition-colors duration-300"
              :class="achievement.unlocked ? 'text-gray-200' : 'text-white/70'"
            >
              {{ achievementDescription(achievement) }}
            </p>

            <!-- Progress Bar -->
            <div
              class="relative h-1.5 w-full overflow-hidden rounded-full border border-white/5
                bg-white/30 backdrop-blur-sm"
            >
              <div
                class="absolute top-0 left-0 h-full shadow-[0_0_8px_currentColor] transition-all
                  duration-1000 ease-out"
                :class="getProgressClass(achievement)"
                :style="{ width: getProgressPercent(achievement) + '%' }"
              ></div>
            </div>

            <!-- Progress Text -->
            <div class="mt-1 flex justify-end" v-if="!achievement.unlocked">
              <span
                class="font-mono text-[10px]"
                :class="achievement.unlocked ? 'text-gray-200' : 'text-white/60'"
              >
                {{
                  achievement.hidden
                    ? "???"
                    : `${achievement.current_progress} / ${achievement.target_progress}`
                }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
  import { computed, onMounted } from "vue";
  import { MenuPage, MenuItem } from "../../ui";
  import { useAchievementStore } from "@/stores/modules/ui/achievement";
  import { achievementTitle, achievementDescription } from "@/utils/achievement-i18n";
  import Icon from "@/components/base/widget/Icon.vue";
  import { Award } from "lucide-vue-next";

  const achievementStore = useAchievementStore();

  const achievementsList = computed(() => {
    return Object.values(achievementStore.allAchievements || {}).sort((a, b) => {
      // 已解锁的排在前面
      if (a.unlocked && !b.unlocked) return -1;
      if (!a.unlocked && b.unlocked) return 1;
      // 未解锁的隐藏成就排最后
      if (!a.unlocked && !b.unlocked) {
        if (a.hidden && !b.hidden) return 1;
        if (!a.hidden && b.hidden) return -1;
      }
      // 如果都解锁了，稀有的排前面
      if (a.unlocked && b.unlocked) {
        if (a.type === "rare" && b.type !== "rare") return -1;
        if (a.type !== "rare" && b.type === "rare") return 1;
      }
      return 0;
    });
  });

  const getCardClass = (ach: any) => {
    if (!ach.unlocked) {
      // 未解锁的隐藏成就：神秘紫色
      if (ach.hidden) {
        return "bg-linear-to-br from-purple-900/30 to-black/60 border-purple-400/40 opacity-80 hover:bg-white/5 backdrop-blur-md";
      }
      // 未解锁：稍微亮一点的背景以提升对比度
      return "bg-black/30 border-white/10 backdrop-blur-md opacity-90 hover:bg-white/5 transition-all";
    }
    if (ach.type === "rare") {
      // 稀有：增强金色光晕和呼吸感
      return "bg-linear-to-br from-yellow-700/30 to-black/60 border-yellow-400 shadow-[0_0_30px_rgba(234,179,8,0.25)] hover:shadow-[0_0_40px_rgba(234,179,8,0.4)] hover:-translate-y-1";
    }
    // 普通：标准玻璃态
    return "bg-black/30 border-white/20 hover:bg-white/5 hover:border-black/5 shadow-lg hover:shadow-emerald-500/10 hover:-translate-y-0.5";
  };

  const getIconClass = (ach: any) => {
    if (!ach.unlocked) {
      if (ach.hidden) return "border-purple-400/40 bg-purple-400/10 text-purple-300";
      return "border-white/20 bg-white/5 text-white/30";
    }

    if (ach.type === "rare") {
      return "border-yellow-400 bg-yellow-400/20 text-yellow-400 shadow-[0_0_15px_rgba(250,204,21,0.4)]";
    }
    return "border-emerald-400 bg-emerald-400/20 text-emerald-400 shadow-[0_0_10px_rgba(52,211,153,0.3)]";
  };

  const getIconSvgClass = (ach: any) => {
    if (!ach.unlocked) {
      if (ach.hidden) return "text-purple-300/80";
      return "text-white/40";
    }
    if (ach.type === "rare") return "text-yellow-400 drop-shadow-[0_0_4px_rgba(250,204,21,0.8)]";
    return "text-emerald-400 drop-shadow-[0_0_4px_rgba(52,211,153,0.6)]";
  };

  const getBadgeClass = (ach: any) => {
    if (ach.type === "rare") return "bg-yellow-500/30 text-yellow-200 border-yellow-400/50";
    return "bg-emerald-500/30 text-emerald-200 border-emerald-400/50";
  };

  const getProgressClass = (ach: any) => {
    if (ach.unlocked) {
      if (ach.type === "rare") return "bg-linear-to-r from-yellow-600 to-yellow-300";
      return "bg-linear-to-r from-emerald-600 to-emerald-300";
    }
    if (ach.hidden) return "bg-linear-to-r from-purple-600 to-purple-300";
    return "bg-white/40";
  };

  const getProgressPercent = (ach: any) => {
    if (ach.unlocked) return 100;
    const current = ach.current_progress || 0;
    const target = ach.target_progress || 1;
    return Math.min(100, Math.max(0, (current / target) * 100));
  };

  onMounted(() => {
    achievementStore.fetchAchievements();
  });
</script>

<style scoped>
  .custom-scrollbar::-webkit-scrollbar {
    width: 6px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background-color: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background-color: rgba(255, 255, 255, 0.2);
  }

  .text-shadow-sm {
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  }
</style>
