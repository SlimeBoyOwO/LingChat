<template>
  <MenuPage>
    <MenuItem title="角色列表">
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
          @saved="handleSettingsSaved"
        />
      </div>
    </MenuItem>

    <MenuItem title="刷新人物列表" size="small">
      <Button type="big" @click="refreshCharacters">点我刷新~</Button>
    </MenuItem>

    <MenuItem title="创意工坊" size="small">
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
import { useUserStore } from '../../../stores/modules/user/user'
import { useUIStore } from '../../../stores/modules/ui/ui'

interface CharacterCard {
  id: number
  title: string
  info: string
  avatar: string
  name: string
  subName: string
  clothes?: Clothes[]
}

const characters = ref<CharacterCard[]>([])
const userId = ref<number>(1)

const gameStore = useGameStore()
const userStore = useUserStore()
const uiStore = useUIStore()

const fetchCharacters = async (): Promise<CharacterCard[]> => {
  try {
    const list = await characterGetAll()
    console.log('list:', list)
    return list.map((char: ApiCharacter) => ({
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
              ? `/api/v1/chat/character/clothes_file/${encodeURIComponent(`${clothes.avatar}\\正常.png`)}`
              : '../pictures/characters/default.png',
          }))
        : [],
    }))
  } catch (error) {
    console.error('获取角色列表失败:', error)
    return []
  }
}

const loadCharacters = async (): Promise<void> => {
  try {
    const characterData = await fetchCharacters()
    characters.value = characterData
  } catch (error) {
    console.error('加载角色失败:', error)
  }
}

const refreshCharacters = async (): Promise<void> => {
  try {
    const response = await fetch('/api/v1/chat/character/refresh_characters', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
    })

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`)
    }

    await response.json()
    await loadCharacters() // 重新加载角色列表

    const tip = uiStore.getRefreshTip('success')
    uiStore.showSuccess({
      title: tip.title,
      message: tip.message,
      duration: 3000,
    })
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
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`)
    }

    await response.json()
  } catch (error) {
    alert('启动失败，请手动去lingchat的discussion网页')
    console.error('打开创意工坊失败:', error)
  }
}

const handleSettingsSaved = () => {
  refreshCharacters()
}

// 初始化加载角色列表
onMounted(() => {
  loadCharacters()
})

// 角色切换时重新加载服装
watch(
  () => gameStore.mainRoleId,
  () => {
    loadCharacters()
  },
)
</script>
