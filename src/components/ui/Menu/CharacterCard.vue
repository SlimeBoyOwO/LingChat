<template>
  <div
    class="group relative flex items-center rounded-2xl border border-white/20 bg-white/10 p-4 backdrop-blur-xl transition-all duration-300 hover:-translate-y-1 hover:border-white/40 hover:shadow-2xl hover:shadow-indigo-500/20"
  >
    <div
      class="text-brand absolute -top-2 -left-2 flex h-6 w-6 -rotate-18 transform items-center justify-center rounded-full shadow-md"
    >
      <Cat :size="20" />
    </div>
    <div class="absolute top-3 right-3 z-10 flex items-center gap-2">
      <RoleExportMenu
        :role-id="id"
        :role-name="name"
      />
      <button
        class="flex items-center justify-center rounded-full bg-black/5 p-1 text-white/60 transition-all hover:rotate-90 hover:bg-white/10 hover:text-white"
        @click.stop="openSettingsModal"
      >
        <Settings :size="24" />
      </button>
    </div>

    <div
      class="flex w-28 shrink-0 flex-col items-center space-y-2 border-r border-white/10 pr-4 md:w-32"
    >
      <div
        class="h-24 w-24 overflow-hidden rounded-full border-2 border-indigo-400/50 shadow-lg md:h-24 md:w-24"
      >
        <img
          :src="avatar"
          :alt="name"
          class="h-full w-full object-cover transition-transform duration-500 group-hover:scale-110"
        />
      </div>
      <span class="bg-brand mt-1 h-1 w-6 rounded-full"></span>
      <h4 class="text-md text-center font-bold tracking-wide text-white drop-shadow-md">
        {{ title }}
      </h4>
    </div>

    <div class="flex h-full min-h-36 flex-1 flex-col justify-between pl-4">
      <div class="pr-8">
        <div class="flex items-center gap-2">
          <div class="text-brand mb-3 text-xl font-bold tracking-widest uppercase opacity-80">
            {{ name }}
          </div>
          <div class="text-brand mb-3 text-sm font-medium tracking-widest uppercase opacity-80">
            {{ subName }}
          </div>
        </div>
        <p class="line-clamp-3 text-base leading-relaxed text-gray-200/90 opacity-80">
          {{ info || '暂无角色介绍' }}
        </p>
      </div>

      <div class="mt-4 flex items-center justify-end gap-2">
        <button
          @click="showDetailModal"
          class="rounded-full border border-white/10 bg-white/10 px-4 py-1.5 text-xs font-semibold text-white transition-all hover:bg-white/20"
        >
          详情
        </button>
        <!-- 加入/退场 切换按钮 -->
        <button
          v-if="!isInScene()"
          @click="joinScene"
          class="rounded-full border border-cyan-400 bg-cyan-500/80 px-4 py-1.5 text-xs font-semibold text-white shadow-lg shadow-cyan-500/20 transition-all hover:bg-cyan-500"
        >
          加入
        </button>
        <button
          v-else-if="!isSelected()"
          @click="leaveScene"
          class="rounded-full border border-red-400 bg-red-500/80 px-4 py-1.5 text-xs font-semibold text-white shadow-lg shadow-red-500/20 transition-all hover:bg-red-500"
        >
          退场
        </button>
        <button
          v-else
          disabled
          class="cursor-not-allowed rounded-full border border-cyan-400/50 bg-cyan-500/50 px-4 py-1.5 text-xs font-semibold text-cyan-200 shadow-lg transition-all"
        >
          已在场
        </button>
        <button
          @click="selectCharacter"
          :class="[
            'rounded-full border px-5 py-1.5 text-xs font-bold shadow-lg transition-all',
            isSelected()
              ? 'border-emerald-400 bg-emerald-500/80 text-white shadow-emerald-500/20'
              : 'border-indigo-500 bg-indigo-600/80 text-white shadow-indigo-500/20 hover:bg-indigo-500',
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
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 backdrop-blur-md"
      @click="closeDetailModal"
    >
      <div
        class="relative flex max-h-[85vh] w-full max-w-4xl flex-col overflow-hidden rounded-3xl border border-white/20 bg-slate-900/40 shadow-2xl backdrop-blur-2xl"
        @click.stop
      >
        <div class="flex items-center gap-4 border-b border-white/10 bg-white/10 p-6">
          <img
            :src="avatar"
            class="h-16 w-16 rounded-2xl border-2 border-indigo-500/50 object-cover"
          />
          <div class="flex-1">
            <h2 class="text-2xl leading-none font-bold text-white">{{ name }}</h2>
            <p class="mt-1 text-sm tracking-tighter text-indigo-300">角色档案与外观配置</p>
          </div>
          <button
            @click="closeDetailModal"
            class="rounded-full p-2 text-white/50 transition-colors hover:bg-red-500/20 hover:text-white"
          >
            <Icon
              icon="close"
              class="h-6 w-6"
            />
          </button>
        </div>

        <div class="flex-1 space-y-8 overflow-y-auto p-6">
          <section>
            <h3 class="mb-4 flex items-center gap-2 font-bold text-white">
              <span class="h-4 w-1 rounded-full bg-orange-500"></span> 基础信息
            </h3>
            <div
              class="mb-3 text-sm font-bold tracking-widest text-gray-200/90 uppercase opacity-80"
            >
              名称：{{ name }}
            </div>
            <div
              class="mb-3 text-sm font-medium tracking-widest text-gray-200/90 uppercase opacity-80"
            >
              所属：{{ subName }}
            </div>
            <div
              class="mb-1 text-sm font-medium tracking-widest text-gray-200/90 uppercase opacity-80"
            >
              介绍：
            </div>
            <p
              class="mb-3 text-sm font-medium tracking-widest text-gray-200/90 uppercase opacity-80"
            >
              {{ info || '暂无角色介绍' }}
            </p>
          </section>

          <section>
            <h3 class="mb-4 flex items-center gap-2 font-bold text-white">
              <span class="h-4 w-1 rounded-full bg-indigo-500"></span> 可选服装
            </h3>
            <div
              v-if="clothes?.length"
              class="flex snap-x gap-4 overflow-x-auto pb-2"
            >
              <div
                v-for="cloth in clothes"
                :key="cloth.title"
                @click="selectClothes(id, cloth.title)"
                class="group w-48 shrink-0 cursor-pointer snap-start"
              >
                <div
                  :class="[
                    'relative mb-2 aspect-1/2 overflow-hidden rounded-xl border-2 transition-all',
                    isClothesSelected(id, cloth.title)
                      ? 'border-indigo-400 shadow-[0_0_15px_rgba(129,140,248,0.5)]'
                      : 'border-white/10',
                  ]"
                >
                  <img
                    :src="cloth.avatar"
                    class="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                  />
                  <div
                    v-if="isClothesSelected(id, cloth.title)"
                    class="absolute top-1 right-1 rounded-full bg-indigo-500 p-1"
                  >
                    <Check class="h-4 w-4"></Check>
                  </div>
                </div>
                <p class="truncate text-center text-xs text-white/80">{{ cloth.title }}</p>
              </div>
            </div>
            <div
              v-else
              class="rounded-xl bg-white/5 p-4 text-center text-sm text-white/40 italic"
            >
              暂无可用服装
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
import { invoke } from '@tauri-apps/api/core'
import { Icon } from '../../base'
import SettingsCharacterInfo from '@/components/settings/pages/SettingsCharacterInfo.vue'
import RoleExportMenu from '@/components/ui/RoleExportMenu.vue'
import {
  selectCharacter as selectCharacterApi,
  selectClothes as selectClothesApi,
} from '@/api/services/character'
import { useGameStore } from '@/stores/modules/game'
import { applyWebInitData } from '@/stores/modules/game/actions'
import { useDialogStore } from '@/stores/modules/ui/dialog'
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
const dialogStore = useDialogStore()

// 逻辑函数
const isSelected = () => gameStore.mainRoleId === props.id
const isClothesSelected = (role_id: number, clothes_name: string) =>
  gameStore.getGameRole(role_id)?.clothesName === clothes_name

const showDetailModal = () => (isDetailVisible.value = true)
const closeDetailModal = () => (isDetailVisible.value = false)

const selectCharacter = async () => {
  const confirmed = await dialogStore.confirm(
    '切换角色会导致当前角色记忆清空，有需要的话不要忘记存档哦',
  )
  if (!confirmed) return

  try {
    const data = await selectCharacterApi(props.id)
    applyWebInitData(gameStore.$state, data)
  } catch (error) {
    console.error('切换角色失败:', error)
  }
}

const selectClothes = async (role_id: number, clothes_name: string) => {
  try {
    // 调用后端API选择衣服
    const response = await selectClothesApi(role_id, clothes_name)

    if (response.success) {
      // 更新本地状态
      const role = gameStore.getGameRole(role_id)
      if (role) {
        role.clothesName = clothes_name
      }
    }
  } catch (error) {
    console.error('选择衣服失败:', error)
    // 可选：显示错误提示
  }
}

// 多人对话：将角色加入场景
const isInScene = () => gameStore.presentRoleIds.includes(props.id)

const joinScene = async () => {
  if (isInScene()) return
  try {
    const result = (await invoke('add_role_to_scene', { roleId: props.id })) as {
      success: boolean
      message: string
    }
    if (result.success) {
      gameStore.presentRoleIds.push(props.id)
      // 确保角色信息已加载
      await gameStore.getOrCreateGameRole(props.id)
    }
    console.log('[CharacterCard] 角色加入场景:', result.message)
  } catch (error) {
    console.error('[CharacterCard] 角色加入场景失败:', error)
  }
}

// 多人对话：将角色移出场景
const leaveScene = async () => {
  if (!isInScene()) return
  try {
    const result = (await invoke('remove_role_from_scene', { roleId: props.id })) as {
      success: boolean
      message: string
    }
    if (result.success) {
      gameStore.presentRoleIds = gameStore.presentRoleIds.filter((id) => id !== props.id)
    }
    console.log('[CharacterCard] 角色退场:', result.message)
  } catch (error) {
    console.error('[CharacterCard] 角色退场失败:', error)
  }
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
