<template>
  <MenuPage>
    <MenuItem title="角色列表">
      <template #header>
        <Rabbit :size="20" />
      </template>
      <div class="grid gap-5 p-3.75 w-full grid-cols-1 md:grid-cols-2">
        <CharacterCard
          v-for="character in characters"
          :key="character.id"
          :id="character.id"
          :avatar="character.avatar"
          :name="character.name"
          :title="character.title"
          :subName="character.subName"
          :info="character.info"
          :clothes="character.clothes || []"
          :resource-folder="character.resourceFolder"
          @saved="handleSettingsSaved"
        />
      </div>
      <div v-if="totalPages > 1" class="flex items-center justify-between px-3 py-2 w-full">
        <button
          class="px-4 py-1.5 text-sm font-medium border-none rounded-lg cursor-pointer bg-[#e9ecef] text-[#495057] transition-all duration-200 hover:bg-(--accent-color) hover:text-white hover:-translate-y-0.5 hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="currentPage <= 1"
          @click="changePage(currentPage - 1)"
        >
          上一页
        </button>
        <span class="text-sm font-medium text-white/80"
          >第 {{ currentPage }} / {{ totalPages }} 页</span
        >
        <button
          class="px-4 py-1.5 text-sm font-medium border-none rounded-lg cursor-pointer bg-[#e9ecef] text-[#495057] transition-all duration-200 hover:bg-(--accent-color) hover:text-white hover:-translate-y-0.5 hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="currentPage >= totalPages"
          @click="changePage(currentPage + 1)"
        >
          下一页
        </button>
      </div>
    </MenuItem>

    <MenuItem title="刷新人物列表" size="small">
      <template #header><RefreshCcw :size="20" /></template>

      <Button type="big" @click="refreshCharacters">点我刷新~</Button>
    </MenuItem>

    <MenuItem title="创意工坊" size="small">
      <template #header>
        <Birdhouse :size="20" />
      </template>
      <Button type="big" @click="openCreativeWeb">进入创意工坊</Button>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { MenuPage } from '../../ui'
import { MenuItem } from '../../ui'
import { Button } from '../../base'

import CharacterCard from '../../ui/Menu/CharacterCard.vue'
import { characterGetAll } from '../../../api/services/character'
import type { Character as ApiCharacter, Clothes } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { Birdhouse, Rabbit, RefreshCcw } from 'lucide-vue-next'

interface CharacterCard {
  id: number
  title: string
  info: string
  avatar: string
  name: string
  subName: string
  clothes?: Clothes[]
  resourceFolder?: string
}

const characters = ref<CharacterCard[]>([])
const currentPage = ref(1)
const totalPages = ref(1)

const gameStore = useGameStore()
const uiStore = useUIStore()

const fetchCharacters = async (page: number): Promise<void> => {
  try {
    const result = await characterGetAll(page)
    totalPages.value = result.total_pages
    characters.value = result.items.map((char: ApiCharacter) => ({
      id: parseInt(char.character_id),
      title: char.title,
      name: char.name,
      subName: char.sub_name,
      info: char.info || '暂无角色描述',
      avatar: char.avatar_path
        ? `/api/v1/chat/character/character_file/${encodeURIComponent(char.avatar_path)}`
        : '../pictures/characters/default.png',
      clothes: char.clothes
        ? char.clothes.map((clothes: Clothes) => ({
            title: clothes.title,
            avatar: clothes.avatar
              ? `/api/v1/chat/character/clothes_file/${encodeURIComponent(`${clothes.avatar}\\头像.png`)}`
              : '../pictures/characters/default.png',
          }))
        : [],
      resourceFolder: char.resource_folder,
    }))
  } catch (error) {
    console.error('获取角色列表失败:', error)
    characters.value = []
  }
}

const loadCharacters = async (): Promise<void> => {
  await fetchCharacters(currentPage.value)
}

const changePage = async (page: number): Promise<void> => {
  currentPage.value = page
  await fetchCharacters(page)
}

const refreshCharacters = async (): Promise<void> => {
  try {
    const response = await fetch('/api/v1/chat/character/refresh_characters', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
    })

    if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`)

    await response.json()
    currentPage.value = 1
    await loadCharacters()

    const tip = uiStore.getRefreshTip('success')
    uiStore.showSuccess({ title: tip.title, message: tip.message, duration: 3000 })
  } catch (error) {
    console.error('刷新失败:', error)
    const tip = uiStore.getRefreshTip('fail')
    uiStore.showError({
      title: tip.title,
      message: (error as Error)?.message || tip.message,
      duration: 3000,
    })
  }
}

const openCreativeWeb = async (): Promise<void> => {
  try {
    const response = await fetch('/api/v1/chat/character/open_web')
    if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`)
    await response.json()
  } catch (error) {
    alert('启动失败，请手动去lingchat的discussion网页')
    console.error('打开创意工坊失败:', error)
  }
}

const handleSettingsSaved = () => {
  refreshCharacters()
}

onMounted(() => {
  loadCharacters()
})

watch(
  () => gameStore.mainRoleId,
  () => {
    currentPage.value = 1
    loadCharacters()
  },
)
</script>
