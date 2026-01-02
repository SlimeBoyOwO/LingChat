<template>
  <MenuPage>
    <MenuItem title="角色列表">
      <CharacterList>
        <div class="character-list character-grid">
          <CharacterCard
            v-for="character in characters"
            :key="character.id"
            :avatar="character.avatar"
            :name="character.title"
            :info="character.info"
            :clothes="character.clothes || []"
            :selectClothes="selectClothes"
            :isClothesSelected="isClothesSelected"
          >
            <template #actions>
              <Button
                type="select"
                :class="['character-select-btn', { selected: isSelected(character.id) }]"
                @click="selectCharacter(character.id)"
                >{{ isSelected(character.id) ? '√ 选中' : '选择' }}</Button
              >
            </template>
          </CharacterCard>
        </div>
      </CharacterList>
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
import CharacterList from '../../ui/Menu/CharacterList.vue'
import { characterGetAll, characterSelect, clothesGetAll } from '../../../api/services/character'
import type { Character as ApiCharacter, Clothes } from '../../../types'
import { useGameStore } from '../../../stores/modules/game'
import { useUserStore } from '../../../stores/modules/user/user'
import { useUIStore } from '../../../stores/modules/ui/ui'

interface CharacterCard {
  id: number
  title: string
  info: string
  avatar: string
  clothes?: Clothes[]
}

const characters = ref<CharacterCard[]>([])
const userId = ref<number>(1)

const gameStore = useGameStore()
const userStore = useUserStore()
const uiStore = useUIStore()

console.log("characters:", characters);
const fetchCharacters = async (): Promise<CharacterCard[]> => {
  try {
    const list = await characterGetAll()
    const clothes = await clothesGetAll()
    return list.map((char: ApiCharacter) => ({
      id: parseInt(char.character_id),
      title: char.title,
      info: char.info || '暂无角色描述',
      avatar: char.avatar_path
        ? `/api/v1/chat/character/character_file/${encodeURIComponent(char.avatar_path)}`
        : '../pictures/characters/default.png',
      clothes: isSelected(parseInt(char.character_id)) ? clothes.map((clothes: Clothes) => ({
        title: clothes.title,
        avatar: clothes.avatar
          ? `/api/v1/chat/clothes/clothes_file/${encodeURIComponent(`${clothes.avatar}\\icon.png`)}`
          : '../pictures/characters/default.png',
      })) : []
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

const updateSelectedStatus = async (): Promise<void> => {
  const userId = '1'
  await gameStore.initializeGame(userStore.client_id, userId)
}

const selectCharacter = async (characterId: number): Promise<void> => {
  try {
    const result = await characterSelect({
      user_id: userId.value.toString(),
      character_id: characterId.toString(),
    })
    updateSelectedStatus()
    
    // 获取切换后角色的文件夹名
    const folderName = result?.character?.folder_name
    
    if (folderName) {
      // 加载新角色的提示配置
      await uiStore.loadCharacterTips(folderName)
      
      // 只有当 public 中存在该角色的 tips.txt 时才显示弹窗
      if (uiStore.tipsAvailable) {
        const successTip = uiStore.getSwitchTip('success')
        uiStore.showSuccess({
          title: successTip.title,
          message: successTip.message,
        })
      } else {
        console.log(`角色 ${folderName} 没有 tips.txt，不显示切换成功弹窗`)
      }
    }
  } catch (error) {
    console.error("切换角色失败:", error)
    
    // 只有当当前角色有 tips.txt 时才显示失败弹窗
    if (uiStore.tipsAvailable) {
      const failTip = uiStore.getSwitchTip('fail')
      uiStore.showError({
        title: failTip.title,
        message: failTip.message,
      })
    }
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
    alert('刷新成功')
    await loadCharacters() // 重新加载角色列表
  } catch (error) {
    alert('刷新失败')
    console.error('刷新失败:', error)
  }
}

const selectClothes = async (clothes_name: string): Promise<void> => {
  gameStore.avatar.clothes_name = clothes_name
  // TODO: send message to AI
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

function isSelected(id: number): boolean {
  return gameStore.avatar.character_id === id
}

function isClothesSelected(clothes_name: string): boolean {
  return gameStore.avatar.clothes_name === clothes_name;
}


// 初始化加载角色列表
onMounted(() => {
  loadCharacters()
})

// 角色切换时重新加载服装
watch(
  () => gameStore.avatar.character_id,
  () => {
    loadCharacters()
  },
)
</script>

<style scoped>
/*=========角色css部分=========*/
/* 角色选择网格布局 */
.character-grid {
  display: grid;
  /* grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); */
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
  /* .character-grid {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  } */
}
</style>
