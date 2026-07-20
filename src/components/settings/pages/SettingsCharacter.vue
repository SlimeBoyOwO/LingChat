<template>
  <MenuPage>
    <MenuItem title="角色列表（切换角色会开始全新对话）">
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
    <RoleArchiveProgress />

    <MenuItem title="打开人物文件夹" size="small">
      <template #header>
        <FolderOpen :size="20" />
      </template>
      <div class="space-y-2">
        <Button type="big" @click="openCharacterFolder">打开人物文件夹</Button>
      </div>
    </MenuItem>

    <MenuItem title="从压缩包导入角色 (.zip / .7z)" size="small">
      <template #header>
        <PackageOpen :size="20" />
      </template>
      <div class="space-y-2">
        <div class="flex flex-col gap-1.5">
          <label class="text-xs text-white/60 font-medium">同名冲突策略</label>
          <select
            v-model="conflictPolicy"
            class="bg-black/20 border border-white/10 rounded-xl px-3 py-2 text-white text-sm outline-none transition-all duration-200"
          >
            <option value="rename">自动重命名（默认）</option>
            <option value="skip">跳过已存在的</option>
            <option value="overwrite">覆盖已存在的</option>
          </select>
        </div>
        <Button type="big" @click="handleImport">选择压缩包导入</Button>
      </div>
    </MenuItem>

    <MenuItem title="刷新人物列表" size="small">
      <template #header>
        <RefreshCcw :size="20" />
      </template>
      <Button type="big" @click="refreshCharacters">点我刷新</Button>
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
import { onMounted, ref, watch } from 'vue'
import { Birdhouse, FolderOpen, PackageOpen, Rabbit, RefreshCcw } from 'lucide-vue-next'
import { convertFileSrc } from '@tauri-apps/api/core'
import { invoke } from '@tauri-apps/api/core'

import CharacterCard from '../../ui/Menu/CharacterCard.vue'
import { Button } from '../../base'
import { MenuItem, MenuPage } from '../../ui'
import { characterGetAll } from '../../../api/services/character'
import { useRoleImportExport } from '../../../composables/useRoleImportExport'
import type { ConflictPolicy } from '../../../api/services/role-archive'
import { useGameStore } from '../../../stores/modules/game'
import { useUIStore } from '../../../stores/modules/ui/ui'
import { useDialogStore } from '../../../stores/modules/ui/dialog'
import type { Character as ApiCharacter, Clothes } from '../../../types'
import RoleArchiveProgress from '@/components/ui/RoleArchiveProgress.vue'

interface CharacterCardData {
  id: number
  title: string
  info: string
  avatar: string
  name: string
  subName: string
  clothes?: Clothes[]
  resourceFolder?: string
}

const characters = ref<CharacterCardData[]>([])
const currentPage = ref(1)
const totalPages = ref(1)
const gameStore = useGameStore()
const uiStore = useUIStore()
const dialogStore = useDialogStore()

const mapCharacter = (char: ApiCharacter): CharacterCardData => {
  return {
    id: parseInt(char.character_id),
    title: char.title,
    name: char.name,
    subName: char.sub_name,
    info: char.info || '暂无角色描述',
    avatar: char.avatar_path ? convertFileSrc(char.avatar_path) : '',
    clothes: char.clothes
      ? char.clothes.map((clothes: Clothes) => ({
          title: clothes.title,
          avatar: clothes.avatar ? convertFileSrc(clothes.avatar) : '',
        }))
      : [],
    resourceFolder: char.resource_folder,
  }
}

const fetchCharacters = async (page: number): Promise<void> => {
  try {
    const result = await characterGetAll(page)
    totalPages.value = result.total_pages
    characters.value = result.items.map(mapCharacter)
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

const { pickAndImport, rescan } = useRoleImportExport()

const conflictPolicy = ref<ConflictPolicy>('rename')

const refreshCharacters = async (): Promise<void> => {
  try {
    await rescan()
  } catch (e) {
    console.error('刷新角色列表失败:', e)
  }
  await loadCharacters()
}

const openCreativeWeb = async (): Promise<void> => {
  uiStore.currentSettingsTab = 'workshop'
}

const handleImport = async () => {
  await pickAndImport(conflictPolicy.value)
  // After import dialog closes (success or cancel), refresh list
  await refreshCharacters()
}

const openCharacterFolder = async () => {
  await invoke('open_characters_folder')
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
