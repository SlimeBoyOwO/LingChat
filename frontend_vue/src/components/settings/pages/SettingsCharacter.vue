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
                  type="nav"
                  :class="['clothes-select-btn', { dim: !isSelected(character.id) }]"
                  @click="() => showClothesPopup(character)"
                  >{{ '选择服装' }}</Button
                >
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

  <!-- Clothes Popup Modal -->
  <Transition name="modal">
    <div v-if="isClothesPopupVisible" class="modal-overlay" @click="closeClothesPopup">
      <div class="modal-container" @click.stop>
        <div class="modal-header">
          <div class="modal-title-section">
            <img
              v-if="selectedCharacter"
              :src="selectedCharacter.avatar"
              :alt="selectedCharacter.title"
              class="modal-character-avatar"
            />
            <div>
              <h2 class="modal-title">{{ selectedCharacter?.title }}</h2>
              <p class="modal-subtitle">选择服装</p>
            </div>
          </div>
          <button class="modal-close-btn" @click="closeClothesPopup">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="24"
              height="24"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>

        <div class="modal-body">
          <div
            v-if="selectedCharacter?.clothes && selectedCharacter.clothes.length > 0"
            class="clothes-grid"
          >
            <div
              v-for="cloth in selectedCharacter.clothes"
              :key="cloth.title"
              class="clothes-item"
              :class="{ 'clothes-item-selected': isClothesSelected(cloth.title) }"
              @click="selectClothes(cloth.title)"
            >
              <div class="clothes-image-wrapper">
                <img :src="cloth.avatar" :alt="cloth.title" class="clothes-image" />
                <div v-if="isClothesSelected(cloth.title)" class="clothes-checkmark">
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="20"
                    height="20"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="3"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  >
                    <polyline points="20 6 9 17 4 12"></polyline>
                  </svg>
                </div>
              </div>
              <p class="clothes-name">{{ cloth.title }}</p>
            </div>
          </div>
          <div v-else class="no-clothes-message">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="64"
              height="64"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path
                d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"
              ></path>
              <line x1="7" y1="7" x2="7.01" y2="7"></line>
            </svg>
            <p>暂无可用服装</p>
          </div>
        </div>
      </div>
    </div>
  </Transition>
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
const isClothesPopupVisible = ref<boolean>(false)
const selectedCharacter = ref<CharacterCard | null>(null)

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
      info: char.info || '暂无角色描述',
      avatar: char.avatar_path
        ? `/api/v1/chat/character/character_file/${encodeURIComponent(char.avatar_path)}`
        : '../pictures/characters/default.png',
      clothes: char.clothes
        ? char.clothes.map((clothes: Clothes) => ({
            title: clothes.title,
            avatar: clothes.avatar
              ? `/api/v1/chat/clothes/clothes_file/${encodeURIComponent(`${clothes.avatar}\\正常.png`)}`
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

const updateSelectedStatus = async (): Promise<void> => {
  const userId = '1'
  await gameStore.initializeGame(userStore.client_id, userId)
}

const showClothesPopup = (character: CharacterCard) => {
  selectedCharacter.value = character
  isClothesPopupVisible.value = true
}

const closeClothesPopup = () => {
  isClothesPopupVisible.value = false
  selectedCharacter.value = null
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
    console.error('切换角色失败:', error)

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
  return gameStore.avatar.clothes_name === clothes_name
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
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
  gap: 20px;
  padding: 15px;
  width: 100%;
}

.clothes-select-btn {
  position: absolute;
  bottom: 15px;
  right: 80px;
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

.dim {
  opacity: 0.3;
  pointer-events: none;
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

/*=========服装弹窗css部分=========*/
/* Modal Overlay */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2;
  padding: 20px;
}

/* Modal Container */
.modal-container {
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.15) 0%, rgba(255, 255, 255, 0.05) 100%);
  backdrop-filter: blur(30px) saturate(180%);
  border-radius: 24px;
  box-shadow:
    0 20px 60px rgba(0, 0, 0, 0.4),
    0 0 1px rgba(255, 255, 255, 0.3) inset;
  border: 1px solid rgba(255, 255, 255, 0.2);
  max-width: 800px;
  width: 100%;
  max-height: 90vh;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* Modal Header */
.modal-header {
  padding: 24px 30px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: linear-gradient(180deg, rgba(255, 255, 255, 0.1) 0%, rgba(255, 255, 255, 0.05) 100%);
}

.modal-title-section {
  display: flex;
  align-items: center;
  gap: 16px;
}

.modal-character-avatar {
  width: 56px;
  height: 56px;
  border-radius: 12px;
  object-fit: cover;
  border: 2px solid rgba(255, 255, 255, 0.3);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
}

.modal-title {
  font-size: 24px;
  font-weight: 700;
  color: #ffffff;
  margin: 0;
  text-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
}

.modal-subtitle {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.7);
  margin: 4px 0 0 0;
}

.modal-close-btn {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  border: none;
  background: rgba(255, 255, 255, 0.1);
  color: #ffffff;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.3s ease;
  backdrop-filter: blur(10px);
}

.modal-close-btn:hover {
  background: rgba(255, 86, 86, 0.8);
  transform: rotate(90deg);
  box-shadow: 0 4px 12px rgba(255, 86, 86, 0.4);
}

/* Modal Body */
.modal-body {
  padding: 30px;
  overflow-y: auto;
  flex: 1;
}

.modal-body::-webkit-scrollbar {
  width: 8px;
}

.modal-body::-webkit-scrollbar-track {
  background: rgba(255, 255, 255, 0.05);
  border-radius: 10px;
}

.modal-body::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 10px;
}

.modal-body::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.3);
}

/* Clothes Grid */
.clothes-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 20px;
}

.clothes-item {
  background: rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  padding: 12px;
  cursor: pointer;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  border: 2px solid rgba(255, 255, 255, 0.1);
  position: relative;
  overflow: hidden;
}

.clothes-item::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(135deg, rgba(121, 217, 255, 0) 0%, rgba(121, 217, 255, 0.1) 100%);
  opacity: 0;
  transition: opacity 0.3s ease;
}

.clothes-item:hover::before {
  opacity: 1;
}

.clothes-item:hover {
  transform: translateY(-4px) scale(1.02);
  border-color: rgba(121, 217, 255, 0.5);
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.3),
    0 0 20px rgba(121, 217, 255, 0.2);
}

.clothes-item-selected {
  border-color: #79d9ff;
  background: rgba(121, 217, 255, 0.15);
  box-shadow:
    0 8px 24px rgba(0, 0, 0, 0.3),
    0 0 20px rgba(121, 217, 255, 0.4);
}

.clothes-image-wrapper {
  position: relative;
  width: 100%;
  aspect-ratio: 1;
  border-radius: 12px;
  overflow: hidden;
  margin-bottom: 10px;
  background: rgba(0, 0, 0, 0.2);
}

.clothes-image {
  width: 100%;
  height: 100%;
  object-fit: contain;
  transition: transform 0.3s ease;
}

.clothes-item:hover .clothes-image {
  transform: scale(1.1);
}

.clothes-checkmark {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 32px;
  height: 32px;
  background: #79d9ff;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #000;
  box-shadow: 0 4px 12px rgba(121, 217, 255, 0.6);
  animation: checkmark-pop 0.3s cubic-bezier(0.68, -0.55, 0.265, 1.55);
}

@keyframes checkmark-pop {
  0% {
    transform: scale(0);
  }
  50% {
    transform: scale(1.2);
  }
  100% {
    transform: scale(1);
  }
}

.clothes-name {
  font-size: 14px;
  font-weight: 600;
  color: #ffffff;
  text-align: center;
  margin: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
}

/* No Clothes Message */
.no-clothes-message {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 60px 20px;
  color: rgba(255, 255, 255, 0.6);
  text-align: center;
}

.no-clothes-message svg {
  margin-bottom: 20px;
  opacity: 0.5;
}

.no-clothes-message p {
  font-size: 18px;
  margin: 0;
}

/* Modal Transition */
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.3s ease;
}

.modal-enter-active .modal-container,
.modal-leave-active .modal-container {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

.modal-enter-from .modal-container,
.modal-leave-to .modal-container {
  transform: scale(0.9) translateY(20px);
  opacity: 0;
}

/* Responsive Design */
@media (max-width: 768px) {
  .modal-container {
    max-width: 95%;
    max-height: 95vh;
    border-radius: 16px;
  }

  .modal-header {
    padding: 16px 20px;
  }

  .modal-title {
    font-size: 20px;
  }

  .modal-character-avatar {
    width: 48px;
    height: 48px;
  }

  .modal-body {
    padding: 20px;
  }

  .clothes-grid {
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: 15px;
  }

  .clothes-item {
    padding: 10px;
  }

  .clothes-name {
    font-size: 12px;
  }
}

@media (max-width: 480px) {
  .clothes-grid {
    grid-template-columns: repeat(2, 1fr);
    gap: 12px;
  }

  .modal-title-section {
    gap: 12px;
  }

  .modal-close-btn {
    width: 36px;
    height: 36px;
  }
}
</style>
