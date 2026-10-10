<template>
  <MenuPage>
    <MenuItem :title="$t('settings.character.list.title')">
      <template #header>
        <Rabbit :size="20" />
      </template>

      <div class="grid w-full grid-cols-1 gap-5 p-3.75 md:grid-cols-2">
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
          :source="character.source"
          :favored="favoredCharacterIds.includes(character.id)"
          @saved="handleSettingsSaved"
          @favoredChange="handleCharacterFavoriteChange"
        />
        <button
          v-if="currentPage === totalPages"
          type="button"
          class="group flex min-h-44 flex-col items-center justify-center gap-4 rounded-2xl border-2 border-dashed border-white/20 bg-white/5 p-4 text-white/60 backdrop-blur-xl transition-all duration-300 hover:-translate-y-1 hover:border-(--accent-color) hover:bg-white/10 hover:text-white focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-(--accent-color)"
          @click="newCharacterVisible = true"
        >
          <span
            class="flex h-16 w-16 items-center justify-center rounded-full bg-white/5 transition-colors group-hover:bg-white/10"
            aria-hidden="true"
          >
            <Plus :size="32" />
          </span>
          <span class="text-lg font-semibold">{{ $t("settings.character.new.title") }}</span>
        </button>
      </div>

      <div v-if="totalPages > 1" class="flex w-full items-center justify-between px-3 py-2">
        <button
          class="cursor-pointer rounded-lg border-none bg-[#e9ecef] px-4 py-1.5 text-sm font-medium text-[#495057] transition-all duration-200 hover:-translate-y-0.5 hover:bg-(--accent-color) hover:text-white hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="currentPage <= 1"
          @click="changePage(currentPage - 1)"
        >
          {{ $t("settings.shared.prevPage") }}
        </button>
        <span class="text-sm font-medium text-white/80">{{
          $t("settings.shared.pageOf", { current: currentPage, total: totalPages })
        }}</span>
        <button
          class="cursor-pointer rounded-lg border-none bg-[#e9ecef] px-4 py-1.5 text-sm font-medium text-[#495057] transition-all duration-200 hover:-translate-y-0.5 hover:bg-(--accent-color) hover:text-white hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="currentPage >= totalPages"
          @click="changePage(currentPage + 1)"
        >
          {{ $t("settings.shared.nextPage") }}
        </button>
      </div>
    </MenuItem>
    <RoleArchiveProgress />
    <NewCharacterModal
      :visible="newCharacterVisible"
      @close="newCharacterVisible = false"
      @created="handleCreated"
    />
    <SettingsCharacterInfo
      :visible="createdEditorVisible"
      :role-id="createdCharacter?.character_id ?? null"
      :title="createdCharacter?.title"
      source="game"
      @close="createdEditorVisible = false"
      @saved="handleSettingsSaved"
    />

    <!-- 打开文件夹依赖桌面端文件管理器，移动端不可用（open_folder 无 Android 分支），整卡隐藏 -->
    <MenuItem v-if="!isAndroid()" :title="$t('settings.character.openFolder.title')" size="small">
      <template #header>
        <FolderOpen :size="20" />
      </template>
      <div class="space-y-2">
        <Button type="big" @click="openCharacterFolder">{{
          $t("settings.character.openFolder.button")
        }}</Button>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.character.import.title')" size="small">
      <template #header>
        <PackageOpen :size="20" />
      </template>
      <div class="space-y-2">
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-medium text-white/60">{{
            $t("settings.character.import.conflictPolicy")
          }}</label>
          <select
            v-model="conflictPolicy"
            class="rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white transition-all duration-200 outline-none"
          >
            <option value="rename">{{ $t("settings.character.import.policyRename") }}</option>
            <option value="skip">{{ $t("settings.character.import.policySkip") }}</option>
            <option value="overwrite">{{ $t("settings.character.import.policyOverwrite") }}</option>
          </select>
        </div>
        <Button type="big" @click="handleImport">{{
          $t("settings.character.import.button")
        }}</Button>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.character.refresh.title')" size="small">
      <template #header>
        <RefreshCcw :size="20" />
      </template>
      <Button type="big" @click="refreshCharacters">{{
        $t("settings.character.refresh.button")
      }}</Button>
    </MenuItem>

    <MenuItem :title="$t('settings.character.workshop.title')" size="small">
      <template #header>
        <Birdhouse :size="20" />
      </template>
      <Button type="big" @click="openCreativeWeb">{{
        $t("settings.character.workshop.enter")
      }}</Button>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Birdhouse, FolderOpen, PackageOpen, Plus, Rabbit, RefreshCcw } from "lucide-vue-next";
import { convertFileSrc } from "@tauri-apps/api/core";
import { invoke } from "@tauri-apps/api/core";

import CharacterCard from "../../ui/Menu/CharacterCard.vue";
import { Button } from "../../base";
import { MenuItem, MenuPage } from "../../ui";
import { characterGetAll, type CreatedCharacter } from "../../../api/services/character";
import NewCharacterModal from "../character/NewCharacterModal.vue";
import SettingsCharacterInfo from "./SettingsCharacterInfo.vue";
import {
  getCharacterFavorites,
  saveCharacterFavorites,
} from "../../../api/services/character-favorites";
import { useRoleImportExport } from "../../../composables/useRoleImportExport";
import type { ConflictPolicy } from "../../../api/services/role-archive";
import { useGameStore } from "../../../stores/modules/game";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { useDialogStore } from "../../../stores/modules/ui/dialog";
import type { Character as ApiCharacter, Clothes } from "../../../types";
import { isAndroid } from "@/utils/platform";
import RoleArchiveProgress from "@/components/ui/RoleArchiveProgress.vue";

interface CharacterCardData {
  id: number;
  title: string;
  info: string;
  avatar: string;
  name: string;
  subName: string;
  clothes?: Clothes[];
  resourceFolder?: string;
  source?: string | null;
}

const characters = ref<CharacterCardData[]>([]);
const newCharacterVisible = ref(false);
const createdEditorVisible = ref(false);
const createdCharacter = ref<CreatedCharacter>();
async function handleCreated(character: CreatedCharacter) {
  createdCharacter.value = character;
  createdEditorVisible.value = true;
  await fetchCharacters();
  const index = allCharacters.value.findIndex((item) => item.id === character.character_id);
  if (index >= 0) paginate(Math.floor(index / PAGE_SIZE) + 1);
}
const allCharacters = ref<CharacterCardData[]>([]);
const currentPage = ref(1);
const totalPages = ref(1);
const favoredCharacterIds = ref<number[]>([]);
const PAGE_SIZE = 6;
const FETCH_ALL_PAGE_SIZE = 1000;

async function loadCharacterFavorites(): Promise<void> {
  try {
    favoredCharacterIds.value = await getCharacterFavorites();
  } catch (error) {
    console.error("读取角色收藏失败:", error);
    favoredCharacterIds.value = [];
  }
}

function paginate(page: number): void {
  // 新建空位占一个卡片位置，始终排在全部角色之后。
  const pages = Math.max(1, Math.ceil((allCharacters.value.length + 1) / PAGE_SIZE));
  totalPages.value = pages;
  currentPage.value = Math.min(Math.max(page, 1), pages);
  const start = (currentPage.value - 1) * PAGE_SIZE;
  characters.value = allCharacters.value.slice(start, start + PAGE_SIZE);
}
function applyCharacterFavoriteOrder(): void {
  const favoredSet = new Set(favoredCharacterIds.value);
  const favoredCharacters = favoredCharacterIds.value
    .map((id) => allCharacters.value.find((character) => character.id === id))
    .filter((character): character is CharacterCardData => Boolean(character));
  const others = allCharacters.value.filter((character) => !favoredSet.has(character.id));
  allCharacters.value = [...favoredCharacters, ...others];
  paginate(currentPage.value);
}

async function handleCharacterFavoriteChange(characterId: number): Promise<void> {
  const previous = favoredCharacterIds.value;
  const next = previous.includes(characterId)
    ? previous.filter((id) => id !== characterId)
    : [...previous, characterId];
  favoredCharacterIds.value = next;
  applyCharacterFavoriteOrder();

  try {
    await saveCharacterFavorites(next);
  } catch (error) {
    console.error("保存角色收藏失败:", error);
    favoredCharacterIds.value = previous;
    applyCharacterFavoriteOrder();
  }
}
const gameStore = useGameStore();
const uiStore = useUIStore();
const router = useRouter();
const dialogStore = useDialogStore();
const { t } = useI18n();

const mapCharacter = (char: ApiCharacter): CharacterCardData => {
  return {
    id: parseInt(char.character_id),
    title: char.title,
    name: char.name,
    subName: char.sub_name,
    info: char.info || t("settings.character.list.noDesc"),
    avatar: char.avatar_path ? convertFileSrc(char.avatar_path) : "",
    clothes: char.clothes
      ? char.clothes.map((clothes: Clothes) => ({
          title: clothes.title,
          avatar: clothes.avatar ? convertFileSrc(clothes.avatar) : "",
        }))
      : [],
    resourceFolder: char.resource_folder,
    source: char.source,
  };
};

const fetchCharacters = async (): Promise<void> => {
  try {
    const result = await characterGetAll(1, FETCH_ALL_PAGE_SIZE);
    allCharacters.value = result.items.map(mapCharacter);
    applyCharacterFavoriteOrder();
  } catch (error) {
    console.error("获取角色列表失败:", error);
    allCharacters.value = [];
    characters.value = [];
    totalPages.value = 1;
  }
};

const loadCharacters = async (): Promise<void> => {
  await loadCharacterFavorites();
  await fetchCharacters();
};

const changePage = (page: number): void => {
  paginate(page);
};

const { pickAndImport, rescan } = useRoleImportExport();

const conflictPolicy = ref<ConflictPolicy>("rename");

const refreshCharacters = async (): Promise<void> => {
  try {
    await rescan();
  } catch (e) {
    console.error("刷新角色列表失败:", e);
  }
  await loadCharacters();
};

const openCreativeWeb = async (): Promise<void> => {
  // 云端创意工坊已迁移为主菜单「创意工坊」二级菜单的独立路由页
  router.push("/workshop");
};

const handleImport = async () => {
  await pickAndImport(conflictPolicy.value);
  // After import dialog closes (success or cancel), refresh list
  await refreshCharacters();
};

const openCharacterFolder = async () => {
  await invoke("open_characters_folder");
};

const handleSettingsSaved = () => {
  refreshCharacters();
};

onMounted(() => {
  loadCharacters();
});

watch(
  () => gameStore.mainRoleId,
  () => {
    currentPage.value = 1;
    loadCharacters();
  },
);
</script>
