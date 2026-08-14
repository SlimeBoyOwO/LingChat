<template>
  <MenuPage>
    <MenuItem :title="$t('settings.character.list.title')">
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
          {{ $t('settings.shared.prevPage') }}
        </button>
        <span class="text-sm font-medium text-white/80"
          >{{ $t('settings.shared.pageOf', { current: currentPage, total: totalPages }) }}</span
        >
        <button
          class="px-4 py-1.5 text-sm font-medium border-none rounded-lg cursor-pointer bg-[#e9ecef] text-[#495057] transition-all duration-200 hover:bg-(--accent-color) hover:text-white hover:-translate-y-0.5 hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:opacity-40 disabled:cursor-not-allowed"
          :disabled="currentPage >= totalPages"
          @click="changePage(currentPage + 1)"
        >
          {{ $t('settings.shared.nextPage') }}
        </button>
      </div>
    </MenuItem>
    <RoleArchiveProgress />

    <!-- 打开文件夹依赖桌面端文件管理器，移动端不可用（open_folder 无 Android 分支），整卡隐藏 -->
    <MenuItem v-if="!isAndroid()" :title="$t('settings.character.openFolder.title')" size="small">
      <template #header>
        <FolderOpen :size="20" />
      </template>
      <div class="space-y-2">
        <Button type="big" @click="openCharacterFolder">{{ $t('settings.character.openFolder.button') }}</Button>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.character.import.title')" size="small">
      <template #header>
        <PackageOpen :size="20" />
      </template>
      <div class="space-y-2">
        <div class="flex flex-col gap-1.5">
          <label class="text-xs text-white/60 font-medium">{{ $t('settings.character.import.conflictPolicy') }}</label>
          <select
            v-model="conflictPolicy"
            class="bg-black/20 border border-white/10 rounded-xl px-3 py-2 text-white text-sm outline-none transition-all duration-200"
          >
            <option value="rename">{{ $t('settings.character.import.policyRename') }}</option>
            <option value="skip">{{ $t('settings.character.import.policySkip') }}</option>
            <option value="overwrite">{{ $t('settings.character.import.policyOverwrite') }}</option>
          </select>
        </div>
        <Button type="big" @click="handleImport">{{ $t('settings.character.import.button') }}</Button>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.character.refresh.title')" size="small">
      <template #header>
        <RefreshCcw :size="20" />
      </template>
      <Button type="big" @click="refreshCharacters">{{ $t('settings.character.refresh.button') }}</Button>
    </MenuItem>

    <MenuItem :title="$t('settings.character.workshop.title')" size="small">
      <template #header>
        <Birdhouse :size="20" />
      </template>
      <Button type="big" @click="openCreativeWeb">{{ $t('settings.character.workshop.enter') }}</Button>
    </MenuItem>

  </MenuPage>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
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
import { isAndroid } from '@/utils/platform'
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
const { t } = useI18n()

const mapCharacter = (char: ApiCharacter): CharacterCardData => {
  return {
    id: parseInt(char.character_id),
    title: char.title,
    name: char.name,
    subName: char.sub_name,
    info: char.info || t('settings.character.list.noDesc'),
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

    // 防御：删除角色后当前页可能超出 total_pages（例如停在第 2 页删掉最后一个），
    // 此时分页条 v-if="totalPages > 1" 整条消失，回退按钮不存在 → 空白列表死锁。
    // 钳制并重取最后一页。
    if (currentPage.value > result.total_pages && result.total_pages > 0) {
      currentPage.value = result.total_pages
      await fetchCharactersInternal(result.total_pages)
      return
    }

    characters.value = result.items.map(mapCharacter)
  } catch (error) {
    console.error('获取角色列表失败:', error)
    characters.value = []
  }
}

// fetchCharacters 的内部调用（钳制回退时用，避免无限递归）
const fetchCharactersInternal = async (page: number): Promise<void> => {
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
