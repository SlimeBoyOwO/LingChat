<template>
  <MenuPage>
    <MenuItem title="🏆 成就列表">
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-5">
        <div
          v-for="achievement in achievementsList"
          :key="achievement.id"
          class="relative flex items-center p-4 rounded-xl border transition-all duration-300 group overflow-hidden"
          :class="getCardClass(achievement)"
        >
          <!-- Rare Effect Background -->
          <div
            v-if="achievement.type === 'rare' && achievement.unlocked"
            class="absolute inset-0 bg-yellow-400/10 blur-2xl animate-pulse"
          ></div>

          <!-- Icon -->
          <div class="relative flex-shrink-0 mr-4 z-10">
            <div
              class="w-16 h-16 rounded-full flex items-center justify-center border-2 transition-all duration-300 shadow-md"
              :class="getIconClass(achievement)"
            >
              <img
                v-if="achievement.imgUrl"
                :src="achievement.imgUrl"
                class="w-10 h-10 object-contain transition-all duration-300"
                :class="{ 'opacity-40': !achievement.unlocked }"
              />
              <Icon v-else icon="achievement" :size="32" :class="getIconSvgClass(achievement)" />
            </div>
          </div>

          <!-- Info -->
          <div class="flex-1 min-w-0 z-10">
            <div class="flex items-center justify-between mb-1.5">
              <h3
                class="text-base font-bold truncate tracking-wide"
                :class="achievement.unlocked ? 'text-white text-shadow-sm' : 'text-white/90'"
              >
                {{ achievement.title }}
              </h3>
              <span
                v-if="achievement.unlocked"
                class="text-[10px] px-2 py-0.5 rounded-full border backdrop-blur-md shadow-sm font-medium"
                :class="getBadgeClass(achievement)"
              >
                {{ achievement.type === 'rare' ? '稀有' : '普通' }}
              </span>
            </div>

            <p
              class="text-xs line-clamp-2 h-8 leading-4 mb-2 transition-colors duration-300"
              :class="achievement.unlocked ? 'text-gray-200' : 'text-white/70'"
            >
              {{ achievement.description }}
            </p>

            <!-- Progress Bar -->
            <div
              class="relative h-1.5 w-full bg-white/30 rounded-full overflow-hidden backdrop-blur-sm border border-white/5"
            >
              <div
                class="absolute top-0 left-0 h-full transition-all duration-1000 ease-out shadow-[0_0_8px_currentColor]"
                :class="getProgressClass(achievement)"
                :style="{ width: getProgressPercent(achievement) + '%' }"
              ></div>
            </div>

            <!-- Progress Text -->
            <div class="flex justify-end mt-1" v-if="!achievement.unlocked">
              <span
                class="text-[10px] font-mono"
                :class="achievement.unlocked ? 'text-gray-200' : 'text-white/60'"
              >
                {{ achievement.current_progress }} / {{ achievement.target_progress }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { useAchievementStore } from '@/stores/modules/ui/achievement'
import Icon from '@/components/base/widget/Icon.vue'

const achievementStore = useAchievementStore()

const achievementsList = computed(() => {
  return Object.values(achievementStore.allAchievements || {}).sort((a, b) => {
    // 已解锁的排在前面
    if (a.unlocked && !b.unlocked) return -1
    if (!a.unlocked && b.unlocked) return 1
    // 如果都解锁了，稀有的排前面
    if (a.unlocked && b.unlocked) {
      if (a.type === 'rare' && b.type !== 'rare') return -1
      if (a.type !== 'rare' && b.type === 'rare') return 1
    }
    return 0
  })
})

const getCardClass = (ach: any) => {
  if (!ach.unlocked) {
    // 未解锁：稍微亮一点的背景以提升对比度
    return 'bg-black/30 border-white/10 backdrop-blur-md opacity-90 hover:bg-white/5 transition-all'
  }
  if (ach.type === 'rare') {
    // 稀有：增强金色光晕和呼吸感
    return 'bg-gradient-to-br from-yellow-700/30 to-black/60 border-yellow-400 shadow-[0_0_30px_rgba(234,179,8,0.25)] hover:shadow-[0_0_40px_rgba(234,179,8,0.4)] hover:-translate-y-1'
  }
  // 普通：标准玻璃态
  return 'bg-black/30 border-white/20 hover:bg-white/5 hover:border-black/5 shadow-lg hover:shadow-emerald-500/10 hover:-translate-y-0.5'
}

const getIconClass = (ach: any) => {
  if (!ach.unlocked) return 'border-white/20 bg-white/5 text-white/30'

  if (ach.type === 'rare') {
    return 'border-yellow-400 bg-yellow-400/20 text-yellow-400 shadow-[0_0_15px_rgba(250,204,21,0.4)]'
  }
  return 'border-emerald-400 bg-emerald-400/20 text-emerald-400 shadow-[0_0_10px_rgba(52,211,153,0.3)]'
}

const getIconSvgClass = (ach: any) => {
  if (!ach.unlocked) return 'text-white/40'
  if (ach.type === 'rare') return 'text-yellow-400 drop-shadow-[0_0_4px_rgba(250,204,21,0.8)]'
  return 'text-emerald-400 drop-shadow-[0_0_4px_rgba(52,211,153,0.6)]'
}

const getBadgeClass = (ach: any) => {
  if (ach.type === 'rare') return 'bg-yellow-500/30 text-yellow-200 border-yellow-400/50'
  return 'bg-emerald-500/30 text-emerald-200 border-emerald-400/50'
}

const getProgressClass = (ach: any) => {
  if (ach.unlocked) {
    if (ach.type === 'rare') return 'bg-gradient-to-r from-yellow-600 to-yellow-300'
    return 'bg-gradient-to-r from-emerald-600 to-emerald-300'
  }
  return 'bg-white/40'
}

const getProgressPercent = (ach: any) => {
  if (ach.unlocked) return 100
  const current = ach.current_progress || 0
  const target = ach.target_progress || 1
  return Math.min(100, Math.max(0, (current / target) * 100))
}

onMounted(() => {
  achievementStore.fetchAchievements()
})
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
