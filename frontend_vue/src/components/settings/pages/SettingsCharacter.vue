<template>
  <MenuPage>
    <MenuItem title="角色列表">
      <CharacterList>
        <div class="character-list character-grid grid gap-5 p-3.75 w-full">
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
            <template #settings>
              <button
                class="settings-icon-btn"
                @click.stop="openSettingsModal(character.id, character.title)"
                title="角色设置"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="18"
                  height="18"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <circle cx="12" cy="12" r="3"></circle>
                  <path
                    d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"
                  ></path>
                </svg>
              </button>
            </template>
            <template #actions>
              <Button
                type="nav"
                :class="[
                  'clothes-select-btn bottom-3.75 right-20 text-white border-none p-[6px_12px] rounded-[20px] cursor-pointer transition-all duration-200 ease-in font-medium hover:bg-[#4a5acf] hover:-translate-y-px hover:shadow-[0_2px_6px_rgba(94,114,228,0.3)]',
                  { dim: !isSelected(character.id) },
                ]"
                @click="() => showClothesPopup(character)"
                >{{ '选择服装' }}</Button
              >
              <Button
                type="select"
                :class="[
                  'character-select-btn bottom-3.75 right-3.75 text-white border-none p-[6px_12px] rounded-[20px] cursor-pointer transition-all duration-200 ease-in font-medium text-[13px] hover:bg-[#4a5acf] hover:-translate-y-px hover:shadow-[0_2px_6px_rgba(94,114,228,0.3)]',
                  { selected: isSelected(character.id) },
                ]"
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

    <!-- Clothes Popup Modal -->
    <Transition name="modal">
      <div
        v-if="isClothesPopupVisible"
        class="fixed top-0 left-0 w-full h-full flex items-center justify-center z-20 p-5 backdrop-blur-sm"
        @click="closeClothesPopup"
      >
        <div
          class="modal-container bg-[linear-gradient(135deg,rgba(255,255,255,0.15)_0%,rgba(255,255,255,0.05)_100%)] backdrop-blur-[30px] backdrop-saturate-180 rounded-3xl shadow-[0_20px_60px_rgba(0,0,0,0.4),inset_0_0_1px_rgba(255,255,255,0.3)] border border-white/20 max-w-200 w-full max-h-[90vh] overflow-hidden flex flex-col md:rounded-2xl"
          @click.stop
        >
          <div
            class="p-[24px_30px] border-b border-white/10 flex items-center justify-between bg-[linear-gradient(180deg,rgba(255,255,255,0.1)_0%,rgba(255,255,255,0.05)_100%)] md:p-[16px_20px]"
          >
            <div class="flex items-center gap-4 max-[480px]:gap-3">
              <img
                v-if="selectedCharacter"
                :src="selectedCharacter.avatar"
                :alt="selectedCharacter.title"
                class="w-14 h-14 rounded-xl object-cover border-2 border-white/30 shadow-[0_4px_12px_rgba(0,0,0,0.2)] md:w-12 md:h-12"
              />
              <div>
                <h2
                  class="text-[24px] font-bold text-white m-0 text-shadow-[0_2px_8px_rgba(0,0,0,0.3)] md:text-[20px]"
                >
                  {{ selectedCharacter?.title }}
                </h2>
                <p class="text-[14px] text-white/70 m-0 mt-1">选择服装</p>
              </div>
            </div>
            <button
              class="w-10 h-10 rounded-full border-none bg-white/10 text-white cursor-pointer flex items-center justify-center transition-all duration-300 ease-in backdrop-blur-[10px] hover:bg-[rgba(255,86,86,0.8)] hover:rotate-90 hover:shadow-[0_4px_12px_rgba(255,86,86,0.4)] max-[480px]:w-9 max-[480px]:h-9"
              @click="closeClothesPopup"
            >
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

          <div class="m p-7.5 overflow-y-auto flex-1 md:p-5">
            <div
              v-if="selectedCharacter?.clothes && selectedCharacter.clothes.length > 0"
              class="grid gap-5 grid-cols-[repeat(auto-fill,minmax(150px,1fr))] md:gap-3.75 md:grid-cols-[repeat(auto-fill,minmax(120px,1fr))] max-[480px]:grid-cols-2 max-[480px]:gap-3"
            >
              <div
                v-for="cloth in selectedCharacter.clothes"
                :key="cloth.title"
                class="clothes-item bg-white/8 border-2 border-white/10 rounded-2xl p-3 cursor-pointer transition-all duration-300 ease-in-out relative overflow-hidden hover:-translate-y-1 hover:scale-[1.02] hover:border-[#79d9ff]/50 hover:shadow-[0_8px_24px_rgba(0,0,0,0.3),0_0_20px_rgba(121,217,255,0.2)] md:p-2.5"
                :class="{ 'clothes-item-selected': isClothesSelected(cloth.title) }"
                @click="selectClothes(cloth.title)"
              >
                <div
                  class="relative w-full aspect-square rounded-xl overflow-hidden mb-2.5 bg-black/20"
                >
                  <img
                    :src="cloth.avatar"
                    :alt="cloth.title"
                    class="w-full h-full object-contain transition-transform duration-300 ease-in hover:scale-110"
                  />
                  <div
                    v-if="isClothesSelected(cloth.title)"
                    class="clothes-checkmark absolute top-2 right-2 w-8 h-8 bg-[#79d9ff] rounded-full flex items-center justify-center text-black shadow-[0_4px_12px_rgba(121,217,255,0.6)]"
                  >
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
                <p
                  class="text-[14px] font-semibold text-white text-center m-0 whitespace-nowrap overflow-hidden text-ellipsis text-shadow-[0_2px_4px_rgba(0,0,0,0.3)] md:text-[12px]"
                >
                  {{ cloth.title }}
                </p>
              </div>
            </div>
            <div
              v-else
              class="no-clothes-message flex flex-col items-center justify-center p-[60px_20px] text-white/60 text-center"
            >
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

    <!-- Character Settings Modal -->
    <CharacterSettingsModal
      :visible="isSettingsModalVisible"
      :role-id="settingsRoleId"
      :title="settingsRoleTitle"
      @close="closeSettingsModal"
      @saved="handleSettingsSaved"
    />
  </MenuPage>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { MenuPage } from '../../ui'
import { MenuItem } from '../../ui'
import { Button } from '../../base'
import CharacterCard from '../../ui/Menu/CharacterCard.vue'
import CharacterList from '../../ui/Menu/CharacterList.vue'
import CharacterSettingsModal from '../modals/CharacterSettingsModal.vue'
import { characterGetAll, characterSelect } from '../../../api/services/character'
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

// Settings Modal State
const isSettingsModalVisible = ref(false)
const settingsRoleId = ref<number | null>(null)
const settingsRoleTitle = ref('')

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

const selectClothes = async (clothes_name: string): Promise<void> => {
  // TODO: 更换衣服的时候，给 AI 发送感知消息或者屏幕识别之类的
  if (gameStore.mainRole) gameStore.mainRole.clothesName = clothes_name
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

const openSettingsModal = (roleId: number, title: string) => {
  settingsRoleId.value = roleId
  settingsRoleTitle.value = title
  isSettingsModalVisible.value = true
}

const closeSettingsModal = () => {
  isSettingsModalVisible.value = false
  settingsRoleId.value = null
}

const handleSettingsSaved = () => {
  loadCharacters() // Reload to show updated info
  uiStore.showSuccess({
    title: '保存成功',
    message: '角色设置已更新',
  })
}

function isSelected(id: number): boolean {
  return gameStore.mainRoleId === id
}

function isClothesSelected(clothes_name: string): boolean {
  if (gameStore.mainRole) return gameStore.mainRole.clothesName == clothes_name
  return false
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

<style scoped>
/*=========角色css部分=========*/
/* 角色选择网格布局 */
.character-grid {
  grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
}

.clothes-select-btn {
  position: absolute;
  background-color: #5e72e4;
  padding: 6px 12px;
  border-radius: 20px;
  font-size: 13px;
  font-weight: 500;
}

.dim {
  opacity: 0.3;
  pointer-events: none;
}

.character-select-btn {
  position: absolute;
  background-color: #5e72e4;
  color: white;
  padding: 6px 12px;
  border-radius: 20px;
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

.settings-icon-btn {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: none;
  background: rgba(0, 0, 0, 0.05);
  color: #666;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.2s;
}

.settings-icon-btn:hover {
  background: white;
  color: #5e72e4;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  transform: rotate(90deg);
}

/* Clothes Grid */

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

.clothes-checkmark {
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
  .character-grid {
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
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
