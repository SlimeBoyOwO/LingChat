<template>
  <MenuPage>
    <MenuItem title="羁绊冒险">
    <template #header>
      <Book :size="20" />
    </template>

    <!-- 如果没有选中角色 -->
    <div v-if="!currentCharacter" class="flex flex-col items-center justify-center py-12 text-gray-400">
      <div class="w-20 h-20 flex items-center justify-center rounded-full bg-gray-800/50 mb-4">
        <Book :size="40" class="text-gray-500" />
      </div>
      <p class="text-lg mb-2">请先在角色页面选择一个角色</p>
      <p class="text-sm text-gray-500 mb-6">选择角色后即可查看其羁绊冒险</p>
      <Button type="big" @click="goToCharacterTab">
        前往角色页面
      </Button>
    </div>

    <!-- 如果已选中角色 -->
    <div v-else class="space-y-4">
      <div class="flex items-center gap-4 p-4 bg-gray-900/50 rounded-xl border border-white/10">
        <img :src="currentCharacterAvatar" class="w-16 h-16 rounded-full object-cover border-2 border-indigo-500/50"
          alt="角色头像" />
        <div class="flex-1 min-w-0">
          <h3 class="text-xl font-bold text-white truncate">{{ currentCharacter.roleName }}</h3>
          <p class="text-gray-400 text-sm truncate">{{ currentCharacter.roleSubTitle || '暂无副标题' }}</p>
        </div>
        <div class="shrink-0">
          <Button type="big" @click="goToCharacterTab">
            切换角色
          </Button>
        </div>
      </div>

      <AdventurePanel :character-folder="characterFolder" />
    </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { Button } from '@/components/base'
import AdventurePanel from '@/components/game/standard/AdventurePanel.vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { Book } from 'lucide-vue-next'

const gameStore = useGameStore()
const uiStore = useUIStore()

// 获取当前主角
const currentCharacter = computed(() => gameStore.mainRole)

// 获取角色头像
const currentCharacterAvatar = computed(() => {
  const folder = characterFolder.value
  if (!folder) return ''
  return `/characters/${folder}/头像.png`
})

// 获取角色文件夹
const characterFolder = computed(() => {
  return uiStore.currentCharacterFolder
})

// 跳转到角色标签页
const goToCharacterTab = () => {
  uiStore.setSettingsTab('character')
}
</script>

<style scoped>
/* 可以添加自定义样式 */
</style>