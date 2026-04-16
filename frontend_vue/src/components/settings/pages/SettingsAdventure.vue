<template>
  <MenuPage>
    <!-- 独立剧本部分 -->
    <MenuItem title="独立剧本">
    <template #header>
      <FileText :size="20" />
    </template>

    <!-- 独立剧本列表 -->
    <div v-if="standaloneScriptsLoading" class="flex flex-col items-center justify-center py-8 text-gray-400">
      <div class="w-12 h-12 border-4 border-brand border-t-transparent rounded-full animate-spin mb-2"></div>
      <p>加载中...</p>
    </div>

    <div v-else-if="standaloneScripts.length === 0"
      class="flex flex-col items-center justify-center py-12 text-gray-400">
      <div class="w-20 h-20 flex items-center justify-center rounded-full bg-gray-800/50 mb-4">
        <FileText :size="40" class="text-gray-500" />
      </div>
      <p class="text-lg mb-2">暂无独立剧本</p>
      <p class="text-sm text-gray-500 mb-6">独立剧本是无需选择角色即可游玩的剧本</p>
    </div>

    <div v-else class="space-y-4">
      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div v-for="script in standaloneScripts" :key="script.script_name"
          class="relative flex flex-col p-4 rounded-xl border transition-all duration-300 group bg-gray-800/50 border-gray-700 hover:bg-gray-800/80 hover:border-brand/50 cursor-pointer">
          <div class="flex items-center justify-between mb-3">
            <h3 class="text-lg font-bold text-white truncate">{{ script.script_name }}</h3>
            <span class="px-3 py-1 rounded-full text-xs font-medium bg-brand/20 text-brand border border-brand/30">
              独立剧本
            </span>
          </div>

          <p v-if="script.description" class="text-sm text-gray-300 mb-4 line-clamp-3 flex-1">
            {{ script.description }}
          </p>
          <p v-else class="text-sm text-gray-500 mb-4 italic">暂无描述</p>

          <div class="flex items-center justify-between mt-auto">
            <span v-if="script.intro_chapter" class="text-xs text-gray-400">
              章节选择（待做）: {{ script.intro_chapter }}
            </span>
            <Button type="select" size="sm" @click.stop="startStandaloneScript(script)">
              开始游玩
            </Button>
          </div>
        </div>
      </div>
    </div>
    </MenuItem>

    <!-- 羁绊冒险部分 -->
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
      <Button type="big" @click="goToCharacterTab"> 前往角色页面 </Button>
    </div>

    <!-- 如果已选中角色 -->
    <div v-else class="space-y-4">
      <div class="flex items-center gap-4 p-4 bg-gray-900/50 rounded-xl border border-white/10">
        <img :src="currentCharacterAvatar" class="w-16 h-16 rounded-full object-cover border-2 border-indigo-500/50"
          alt="角色头像" />
        <div class="flex-1 min-w-0">
          <h3 class="text-xl font-bold text-white truncate">{{ currentCharacter.roleName }}</h3>
          <p class="text-gray-400 text-sm truncate">
            {{ currentCharacter.roleSubTitle || '暂无副标题' }}
          </p>
        </div>
        <div class="shrink-0">
          <Button type="big" @click="goToCharacterTab"> 切换角色 </Button>
        </div>
      </div>

      <AdventurePanel :character-folder="characterFolder" />
    </div>
    </MenuItem>

    <MenuItem title="创意工坊" size="small">
    <template #header>
      <Birdhouse :size="20" />
    </template>
    <Button type="big" @click="openCreativeWeb">进入创意工坊</Button>
    </MenuItem>

    <MenuItem title="创建自己的剧本" size="small">
    <template #header>
      <UserPlus :size="20" />
    </template>
    <div class="space-y-2">
      <Button type="big" @click="openGuideWeb">访问指南网站</Button>
    </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { MenuPage, MenuItem } from '../../ui'
import { Button } from '@/components/base'
import AdventurePanel from '@/components/game/standard/AdventurePanel.vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { Birdhouse, Book, FileText, UserPlus } from 'lucide-vue-next'
import {
  getStandaloneScriptList,
  startStandaloneScript as startStandaloneScriptApi,
} from '@/api/services/script-info'
import type { ScriptSummary } from '@/api/services/script-info'

const gameStore = useGameStore()
const uiStore = useUIStore()

// 独立剧本相关状态
const standaloneScripts = ref<ScriptSummary[]>([])
const standaloneScriptsLoading = ref(true)

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

// 开始游玩独立剧本
const startStandaloneScript = async (script: ScriptSummary) => {
  try {
    await startStandaloneScriptApi(script.script_name)
    // 可选：关闭设置面板，开始剧本
    uiStore.showSettings = false
  } catch (error) {
    console.error('启动独立剧本失败:', error)
  }
}

// 获取独立剧本列表
const fetchStandaloneScripts = async () => {
  try {
    standaloneScriptsLoading.value = true
    const scripts = await getStandaloneScriptList()
    standaloneScripts.value = scripts
  } catch (error) {
    console.error('获取独立剧本列表失败:', error)
    standaloneScripts.value = []
  } finally {
    standaloneScriptsLoading.value = false
  }
}

const openCreativeWeb = async (): Promise<void> => {
  try {
    const response = await fetch('/api/v1/chat/character/open_web')
    if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`)
    await response.json()
  } catch (error) {
    alert('启动失败，请手动打开 LingChat 的 discussion 页面')
    console.error('打开创意工坊失败:', error)
  }
}

const openGuideWeb = async (): Promise<void> => {
  // 弹出 提示框 显示网页


}

// 组件挂载时获取独立剧本列表
onMounted(() => {
  fetchStandaloneScripts()
})
</script>

<style scoped>
/* 可以添加自定义样式 */
</style>
