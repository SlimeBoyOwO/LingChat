<template>
  <MenuPage>
    <!-- 独立剧本部分 -->
    <MenuItem :title="$t('settings.adventure.standalone.title')">
      <template #header>
        <FileText :size="20" />
      </template>

      <!-- 独立剧本列表 -->
      <div
        v-if="standaloneScriptsLoading"
        class="flex flex-col items-center justify-center py-8 text-gray-400"
      >
        <div
          class="w-12 h-12 border-4 border-brand border-t-transparent rounded-full animate-spin mb-2"
        ></div>
        <p>{{ $t('settings.shared.loading') }}</p>
      </div>

      <div
        v-else-if="standaloneScripts.length === 0"
        class="flex flex-col items-center justify-center py-12 text-gray-400"
      >
        <div class="w-20 h-20 flex items-center justify-center rounded-full bg-gray-800/50 mb-4">
          <FileText :size="40" class="text-gray-500" />
        </div>
        <p class="text-lg mb-2">{{ $t('settings.adventure.standalone.empty') }}</p>
        <p class="text-sm text-gray-500 mb-6">{{ $t('settings.adventure.standalone.emptyDesc') }}</p>
      </div>

      <div v-else class="space-y-4">
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div
            v-for="script in standaloneScripts"
            :key="script.script_name"
            class="relative flex flex-col p-4 rounded-xl border transition-all duration-300 group bg-gray-800/50 border-gray-700 hover:bg-gray-800/80 hover:border-brand/50 cursor-pointer"
          >
            <div class="flex items-center justify-between mb-3">
              <h3 class="text-lg font-bold text-white truncate">{{ script.script_name }}</h3>
              <span
                class="px-3 py-1 rounded-full text-xs font-medium bg-brand/20 text-brand border border-brand/30"
              >
                {{ $t('settings.adventure.standalone.badge') }}
              </span>
            </div>

            <p v-if="script.description" class="text-sm text-gray-300 mb-4 line-clamp-3 flex-1">
              {{ script.description }}
            </p>
            <p v-else class="text-sm text-gray-500 mb-4 italic">{{ $t('settings.adventure.standalone.noDesc') }}</p>

            <div class="flex items-center justify-between mt-auto">
              <span v-if="script.intro_chapter" class="text-xs text-gray-400">
                {{ $t('settings.adventure.standalone.chapterSelect', { chapter: script.intro_chapter }) }}
              </span>
              <Button type="select" size="sm" @click.stop="startStandaloneScript(script)">
                {{ $t('settings.adventure.standalone.play') }}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </MenuItem>

    <!-- 羁绊冒险部分 -->
    <MenuItem :title="$t('settings.adventure.bond.title')">
      <template #header>
        <Book :size="20" />
      </template>

      <!-- 如果没有选中角色 -->
      <div
        v-if="!currentCharacter"
        class="flex flex-col items-center justify-center py-12 text-gray-400"
      >
        <div class="w-20 h-20 flex items-center justify-center rounded-full bg-gray-800/50 mb-4">
          <Book :size="40" class="text-gray-500" />
        </div>
        <p class="text-lg mb-2">{{ $t('settings.adventure.bond.noCharacter') }}</p>
        <p class="text-sm text-gray-500 mb-6">{{ $t('settings.adventure.bond.noCharacterDesc') }}</p>
        <Button type="big" @click="goToCharacterTab"> {{ $t('settings.adventure.bond.goCharacter') }} </Button>
      </div>

      <!-- 如果已选中角色 -->
      <div v-else class="space-y-4">
        <div class="flex items-center gap-4 p-4 bg-gray-900/50 rounded-xl border border-white/10">
          <img
            :src="currentCharacterAvatar"
            class="w-16 h-16 rounded-full object-cover border-2 border-indigo-500/50"
            :alt="$t('settings.adventure.bond.avatarAlt')"
          />
          <div class="flex-1 min-w-0">
            <h3 class="text-xl font-bold text-white truncate">{{ currentCharacter.roleName }}</h3>
            <p class="text-gray-400 text-sm truncate">
              {{ currentCharacter.roleSubTitle || $t('settings.adventure.bond.noSubtitle') }}
            </p>
          </div>
          <div class="shrink-0">
            <Button type="big" @click="goToCharacterTab"> {{ $t('settings.adventure.bond.switchCharacter') }} </Button>
          </div>
        </div>

        <div v-if="gameStore.mainRole">
          <AdventurePanel :character-folder="gameStore.mainRole.character_folder" />
        </div>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.adventure.workshop.title')" size="small">
      <template #header>
        <Birdhouse :size="20" />
      </template>
      <Button type="big" @click="openCreativeWeb">{{ $t('settings.adventure.workshop.enter') }}</Button>
    </MenuItem>

    <MenuItem :title="$t('settings.adventure.createScript.title')" size="small">
      <template #header>
        <UserPlus :size="20" />
      </template>
      <div class="space-y-2">
        <Button type="big" @click="openGuideWeb">{{ $t('settings.adventure.createScript.guide') }}</Button>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, watch } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { openUrl } from '@tauri-apps/plugin-opener'
import { MenuPage, MenuItem } from '../../ui'
import { Button } from '@/components/base'
import AdventurePanel from './Adeventure/AdventurePanel.vue'
import { useGameStore } from '@/stores/modules/game'
import { useUIStore } from '@/stores/modules/ui/ui'
import { getAvatarFile } from '@/api/services/character'
import { Birdhouse, Book, FileText, UserPlus } from 'lucide-vue-next'
import { getStandaloneScriptList, startScript as startScriptApi } from '@/api/services/script-info'
import type { ScriptSummary } from '@/api/services/script-info'

const gameStore = useGameStore()
const uiStore = useUIStore()

// 独立剧本相关状态
const standaloneScripts = ref<ScriptSummary[]>([])
const standaloneScriptsLoading = ref(true)

// 获取当前主角
const currentCharacter = computed(() => gameStore.mainRole)

// 获取角色头像
const currentCharacterAvatar = ref('')

async function updateCharacterAvatar() {
  if (gameStore.mainRole?.character_folder) {
    try {
      const path = await getAvatarFile(
        gameStore.mainRole.character_folder,
        gameStore.mainRole.clothesName,
      )
      currentCharacterAvatar.value = convertFileSrc(path)
    } catch {
      currentCharacterAvatar.value = ''
    }
  } else {
    currentCharacterAvatar.value = ''
  }
}

watch(() => gameStore.mainRole?.character_folder, updateCharacterAvatar, { immediate: true })

// 跳转到角色标签页
const goToCharacterTab = () => {
  uiStore.setSettingsTab('character')
}

// 开始游玩独立剧本
const startStandaloneScript = async (script: ScriptSummary) => {
  try {
    await startScriptApi(script.script_name)
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

const openCreativeWeb = () => {
  // 创意工坊已内置为设置页的独立标签，直接切换即可
  uiStore.setSettingsTab('workshop')
}

const openGuideWeb = () => {
  openUrl('https://slimeboyowo.github.io/LingBlog/blog/projects/ling-chat/script-events')
}

// 组件挂载时获取独立剧本列表
onMounted(() => {
  fetchStandaloneScripts()
})
</script>

<style scoped>
/* 可以添加自定义样式 */
</style>
