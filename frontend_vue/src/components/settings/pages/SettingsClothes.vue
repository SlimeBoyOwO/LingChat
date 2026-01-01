<template>
  <MenuPage>
    <MenuItem title="服装列表">
      <CharacterList>
        <div class="character-list character-grid">
          <CharacterCard v-for="cloth in clothes" :avatar="cloth.avatar" :name="cloth.title">
            <template #actions>
              <Button
                type="select"
                :class="['character-select-btn', { selected: isSelected(cloth.title) }]"
                @click="selectClothes(cloth.title)"
                >{{ isSelected(cloth.title) ? '√ 选中' : '选择' }}</Button
              >
            </template>
          </CharacterCard>
        </div>
      </CharacterList>
    </MenuItem>

    <MenuItem title="刷新服装列表" size="small">
      <Button type="big" @click="refreshCharacters">点我刷新~</Button>
    </MenuItem>

    <MenuItem title="创意工坊" size="small">
      <Button type="big" @click="openCreativeWeb">进入创意工坊</Button>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { MenuPage } from '../../ui'
import { MenuItem } from '../../ui'
import { Button } from '../../base'
import CharacterCard from '../../ui/Menu/CharacterCard.vue'
import CharacterList from '../../ui/Menu/CharacterList.vue'
import { characterGetAll, characterSelect } from '../../../api/services/character'
import type { Clothes } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUserStore } from '../../../stores/modules/user/user'
import { getAllClothes } from '@/api/services/clothes'

interface ClothesCard {
  title: string
  avatar: string
}

const clothes = ref<ClothesCard[]>([])
const userId = ref<number>(1)

const gameStore = useGameStore()
const userStore = useUserStore()

const currentClothes = gameStore.clothes_name

const fetchClothes = async (): Promise<ClothesCard[]> => {
  try {
    const list = await getAllClothes()
    return list.map((clothes: Clothes) => ({
      title: clothes.title,
      avatar: clothes.avatar
        ? `/api/v1/chat/clothes/clothes_file/${encodeURIComponent(`${clothes.avatar}\\正常.png`)}`
        : '../pictures/characters/default.png',
    }))
  } catch (error) {
    console.error('获取服装列表失败:', error)
    return []
  }
}

const loadClothes = async (): Promise<void> => {
  try {
    const new_clothes = await fetchClothes()
    clothes.value = new_clothes
    console.log('clothes:', clothes)
  } catch (error) {
    console.error('加载服装失败:', error)
  }
}

const updateSelectedStatus = async (): Promise<void> => {
  const userId = '1'
  await gameStore.initializeGame(userStore.client_id, userId)
}

const selectClothes = async (clothes_name: string): Promise<void> => {
  // To do : update settings
}

const refreshCharacters = async (): Promise<void> => {
  try {
    await loadClothes() // 重新加载角色列表
  } catch (error) {
    alert('刷新失败')
    console.error('刷新失败:', error)
  }
}

const openCreativeWeb = async (): Promise<void> => {
  try {
    const response = await fetch('/api/v1/chat/clothes/open_web')
    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`)
    }

    await response.json()
  } catch (error) {
    alert('启动失败，请手动去lingchat的discussion网页')
    console.error('打开创意工坊失败:', error)
  }
}

function isSelected(name: string): boolean {
  return gameStore.avatar.clothes_name === name
}

// 初始化加载角色列表
onMounted(() => {
  loadClothes()
})
</script>

<style scoped>
/*=========角色css部分=========*/
/* 角色选择网格布局 */
.character-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 20px;
  padding: 15px;
  width: 100%;
}

.character-select-btn {
  position: absolute;
  bottom: 15px;
  right: 15px;
  background-color: #5e72e4;
  color: white;
  border: none;
  padding: 6px 12px;
  border-radius: 20px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 13px;
  font-weight: 500;
}

.character-select-btn:hover {
  background-color: #4a5acf;
  transform: translateY(-1px);
  box-shadow: 0 2px 6px rgba(94, 114, 228, 0.3);
}

.selected {
  background-color: #10b981 !important;
}

@media (max-width: 768px) {
  .character-grid {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  }
}
</style>
