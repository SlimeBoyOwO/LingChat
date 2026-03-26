<template>
  <div
    class="relative flex items-center p-4 rounded-2xl transition-all duration-300 group bg-white/10 backdrop-blur-xl border border-white/20 hover:border-white/40 hover:-translate-y-1 hover:shadow-2xl hover:shadow-indigo-500/20"
  >
    <div
      class="absolute -top-2 -left-2 w-6 h-6 rounded-full flex items-center justify-center text-brand shadow-md transform -rotate-18"
    >
      <Cat :size="20" />
    </div>
    <button
      class="absolute top-3 right-3 p-1 z-10 rounded-full bg-black/5 text-white/60 hover:text-white hover:bg-white/10 transition-all hover:rotate-90"
      @click.stop="openSettingsModal"
    >
      <Settings />
    </button>

    <div
      class="flex flex-col items-center w-28 md:w-32 shrink-0 space-y-2 border-r border-white/10 pr-4"
    >
      <div
        class="w-24 h-24 md:w-24 md:h-24 rounded-full overflow-hidden border-2 border-indigo-400/50 shadow-lg"
      >
        <img
          :src="avatar"
          :alt="name"
          class="w-full h-full object-cover group-hover:scale-110 transition-transform duration-500"
        />
      </div>
      <span class="w-6 h-1 bg-brand rounded-full mt-1"></span>
      <h4 class="font-bold text-white text-md tracking-wide drop-shadow-md text-center">
        {{ title }}
      </h4>
    </div>

    <div class="flex-1 pl-4 flex flex-col h-full justify-between min-h-36">
      <div class="pr-8">
        <div class="flex gap-2 items-center">
          <div class="text-xl text-brand font-bold uppercase tracking-widest mb-3 opacity-80">
            {{ name }}
          </div>
          <div class="text-sm font-medium text-brand uppercase tracking-widest mb-3 opacity-80">
            {{ subName }}
          </div>
        </div>
        <p class="text-base text-gray-200/90 leading-relaxed line-clamp-3 opacity-80">
          {{ info || '暂无角色介绍' }}
        </p>
      </div>

      <div class="flex justify-end items-center gap-2 mt-4">
        <button
          @click="showDetailModal"
          class="px-4 py-1.5 rounded-full bg-white/10 hover:bg-white/20 text-white text-xs font-semibold border border-white/10 transition-all"
        >
          详情
        </button>
        <button
          @click="selectCharacter"
          :class="[
            'px-5 py-1.5 rounded-full text-xs font-bold transition-all border shadow-lg',
            isSelected()
              ? 'bg-emerald-500/80 border-emerald-400 text-white shadow-emerald-500/20'
              : 'bg-indigo-600/80 border-indigo-500 text-white hover:bg-indigo-500 shadow-indigo-500/20',
          ]"
        >
          {{ isSelected() ? '√ 已选中' : '选择' }}
        </button>
      </div>
    </div>
  </div>

  <Transition name="modal">
    <div
      v-if="isDetailVisible"
      class="fixed inset-0 z-50 flex items-center justify-center p-4 backdrop-blur-md bg-black/40"
      @click="closeDetailModal"
    >
      <div
        class="relative w-full max-w-4xl max-h-[85vh] overflow-hidden rounded-3xl border border-white/20 bg-slate-900/40 backdrop-blur-2xl shadow-2xl flex flex-col"
        @click.stop
      >
        <div class="p-6 flex items-center gap-4 bg-white/10 border-b border-white/10">
          <img
            :src="avatar"
            class="w-16 h-16 rounded-2xl object-cover border-2 border-indigo-500/50"
          />
          <div class="flex-1">
            <h2 class="text-2xl font-bold text-white leading-none">{{ name }}</h2>
            <p class="text-indigo-300 text-sm mt-1 tracking-tighter">角色档案与外观配置</p>
          </div>
          <button
            @click="closeDetailModal"
            class="p-2 hover:bg-red-500/20 text-white/50 hover:text-white rounded-full transition-colors"
          >
            <Icon icon="close" class="w-6 h-6" />
          </button>
        </div>

        <div class="flex-1 overflow-y-auto p-6 space-y-8">
          <section>
            <h3 class="text-white font-bold mb-4 flex items-center gap-2">
              <span class="w-1 h-4 bg-orange-500 rounded-full"></span> 基础信息
            </h3>
            <div
              class="text-sm text-gray-200/90 font-bold uppercase tracking-widest mb-3 opacity-80"
            >
              名称：{{ name }}
            </div>
            <div
              class="text-sm font-medium text-gray-200/90 uppercase tracking-widest mb-3 opacity-80"
            >
              所属：{{ subName }}
            </div>
            <div
              class="text-sm font-medium text-gray-200/90 uppercase tracking-widest mb-1 opacity-80"
            >
              介绍：
            </div>
            <p
              class="text-sm font-medium text-gray-200/90 uppercase tracking-widest mb-3 opacity-80"
            >
              {{ info || '暂无角色介绍' }}
            </p>
          </section>

          <section>
            <h3 class="text-white font-bold mb-4 flex items-center gap-2">
              <span class="w-1 h-4 bg-indigo-500 rounded-full"></span> 可选服装
            </h3>
            <div v-if="clothes?.length" class="flex gap-4 overflow-x-auto pb-2 snap-x">
              <div
                v-for="cloth in clothes"
                :key="cloth.title"
                @click="selectClothes(cloth.title)"
                class="group shrink-0 w-32 snap-start cursor-pointer"
              >
                <div
                  :class="[
                    'relative aspect-3/4 rounded-xl overflow-hidden border-2 transition-all mb-2',
                    isClothesSelected(cloth.title)
                      ? 'border-indigo-400 shadow-[0_0_15px_rgba(129,140,248,0.5)]'
                      : 'border-white/10',
                  ]"
                >
                  <img
                    :src="cloth.avatar"
                    class="w-full h-full object-cover group-hover:scale-110 transition-duration-500"
                  />
                  <div
                    v-if="isClothesSelected(cloth.title)"
                    class="absolute top-1 right-1 bg-indigo-500 rounded-full p-1"
                  >
                    <Check class="w-4 h-4"></Check>
                  </div>
                </div>
                <p class="text-xs text-center text-white/80 truncate">{{ cloth.title }}</p>
              </div>
            </div>
            <div v-else class="text-white/40 text-sm italic p-4 bg-white/5 rounded-xl text-center">
              暂无可用服装
            </div>
          </section>

          <section>
            <h3 class="text-white font-bold mb-4 flex items-center gap-2">
              <span class="w-1 h-4 bg-emerald-500 rounded-full"></span> 羁绊档案（实验）
            </h3>
            <div v-if="resourceFolder" class="h-96">
              <AdventurePanel :character-folder="resourceFolder" />
            </div>
            <div v-else class="text-white/40 text-sm italic p-4 bg-white/5 rounded-xl text-center">
              暂无羁绊冒险数据
            </div>
          </section>
        </div>
      </div>
    </div>
  </Transition>

  <SettingsCharacterInfo
    :visible="isSettingsModalVisible"
    :role-id="id"
    :title="name"
    @close="closeSettingsModal"
    @saved="handleSettingsSaved"
  />
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { Icon } from '../../base'
import SettingsCharacterInfo from '@/components/settings/pages/SettingsCharacterInfo.vue'
import AdventurePanel from '@/components/game/standard/AdventurePanel.vue'
import { characterSelect } from '@/api/services/character'
import { useGameStore } from '@/stores/modules/game'
import { useUserStore } from '@/stores/modules/user/user'
import { Settings } from 'lucide-vue-next'
import { Cat, Check } from 'lucide-vue-next'
import type { Clothes } from '@/types'

interface CharacterProps {
  id: number
  avatar?: string
  name?: string
  title?: string
  subName?: string
  info?: string
  clothes?: Clothes[]
  resourceFolder?: string
}

const props = withDefaults(defineProps<CharacterProps>(), {
  avatar: '',
  name: 'Unknown',
  info: '',
  clothes: () => [],
  resourceFolder: '',
})

const emit = defineEmits(['saved'])

// 状态管理
const isDetailVisible = ref(false)
const isSettingsModalVisible = ref(false)

const gameStore = useGameStore()
const userStore = useUserStore()

// 逻辑函数
const isSelected = () => gameStore.mainRoleId === props.id
const isClothesSelected = (clothes_name: string) => gameStore.mainRole?.clothesName === clothes_name

const showDetailModal = () => (isDetailVisible.value = true)
const closeDetailModal = () => (isDetailVisible.value = false)

const selectCharacter = async () => {
  const confirmed = window.confirm('切换角色会导致当前角色记忆清空，有需要的话不要忘记存档哦')
  if (!confirmed) return

  try {
    await characterSelect({
      user_id: userStore.user_id.toString(),
      character_id: props.id.toString(),
    })
    await gameStore.initializeGame(userStore.client_id, userStore.user_id.toString())
  } catch (error) {
    console.error('切换角色失败:', error)
  }
}

const selectClothes = async (clothes_name: string) => {
  if (gameStore.mainRole) gameStore.mainRole.clothesName = clothes_name
}

const openSettingsModal = () => (isSettingsModalVisible.value = true)
const closeSettingsModal = () => (isSettingsModalVisible.value = false)
const handleSettingsSaved = () => emit('saved')
</script>

<style scoped>
/* 仅保留必要的动画定义，其余全部由 Tailwind 处理 */
.modal-enter-active,
.modal-leave-active {
  transition: all 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
  transform: scale(0.95) translateY(10px);
}

/* 隐藏滚动条但允许滚动 */
.overflow-x-auto::-webkit-scrollbar,
.overflow-y-auto::-webkit-scrollbar {
  display: none;
}
</style>
