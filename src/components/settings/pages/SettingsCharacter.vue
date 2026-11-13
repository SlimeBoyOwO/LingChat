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
          class="group flex min-h-44 flex-col items-center justify-center gap-4 rounded-2xl border-2 border-dashed border-white/20 bg-white/5 p-4 text-white/60 transition-all duration-300 hover:-translate-y-1 hover:border-(--accent-color) hover:bg-white/10 hover:text-white focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-(--accent-color)"
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

    <!-- 我的身份：换身份 = 开一段新对话（本局已开始则不能原地换） -->
    <MenuItem :title="$t('settings.identity.title')">
      <template #header>
        <User :size="20" />
      </template>

      <div class="space-y-3">
        <p class="text-xs leading-relaxed text-white/50">{{ $t("settings.identity.hint") }}</p>

        <!-- 锁定原因 -->
        <p v-if="identityScriptBlocked" class="text-xs leading-relaxed text-red-300/80">
          {{ $t("settings.identity.scriptBlocked") }}
        </p>

        <div v-if="identityLoading" class="text-sm text-white/50">
          {{ $t("settings.shared.loading") }}
        </div>

        <p v-else-if="identities.length === 0" class="text-sm text-white/50">
          {{ $t("settings.identity.empty") }}
        </p>

        <div v-else class="flex flex-col gap-3">
          <div
            v-for="item in pagedIdentities"
            :key="item.id"
            class="flex items-center gap-4 rounded-2xl border border-white/20 bg-white/10 p-4 backdrop-blur-xl transition-all duration-300 hover:-translate-y-0.5 hover:border-white/40 hover:shadow-2xl hover:shadow-indigo-500/20"
          >
            <div
              class="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-full border-2 border-indigo-400/50 bg-indigo-500/20 text-lg font-bold text-white/80"
            >
              {{ Array.from(item.name)[0] || "?" }}
            </div>
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <span class="truncate text-base font-bold tracking-wide text-white">{{
                  item.name
                }}</span>
                <span v-if="item.subtitle" class="truncate text-xs text-white/50">{{
                  item.subtitle
                }}</span>
                <span
                  v-if="item.is_current"
                  class="shrink-0 rounded-full bg-[#79d9ff] px-2 py-0.5 text-[10px] text-white"
                  >{{ $t("settings.identity.current") }}</span
                >
              </div>
            </div>

            <div class="flex shrink-0 flex-wrap items-center justify-end gap-2">
              <button
                v-if="!item.is_current"
                class="rounded-full border border-cyan-400 bg-cyan-500/80 px-4 py-1.5 text-xs font-semibold text-white shadow-lg shadow-cyan-500/20 transition-all hover:bg-cyan-500 disabled:cursor-not-allowed disabled:opacity-50"
                :disabled="identityScriptBlocked"
                :title="identityScriptBlocked ? $t('settings.identity.scriptBlocked') : ''"
                @click="switchIdentity(item)"
              >
                {{ $t("settings.identity.switch") }}
              </button>
              <button
                v-if="!item.is_current"
                class="rounded-full border border-white/10 bg-white/10 px-4 py-1.5 text-xs font-semibold text-white transition-all hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-50"
                :disabled="identityScriptBlocked"
                :title="identityScriptBlocked ? $t('settings.identity.scriptBlocked') : ''"
                @click="startNewGame(item)"
              >
                {{ $t("settings.identity.newGame") }}
              </button>
              <button
                class="rounded-full border border-indigo-500 bg-indigo-600/80 px-4 py-1.5 text-xs font-semibold text-white shadow-lg shadow-indigo-500/20 transition-all hover:bg-indigo-500"
                @click="openIdentityEditor(item.id)"
              >
                {{ $t("settings.identity.edit") }}
              </button>
            </div>
          </div>
        </div>

        <div
          v-if="identityTotalPages > 1"
          class="flex w-full items-center justify-between px-3 py-2"
        >
          <button
            class="cursor-pointer rounded-lg border-none bg-[#e9ecef] px-4 py-1.5 text-sm font-medium text-[#495057] transition-all duration-200 hover:-translate-y-0.5 hover:bg-(--accent-color) hover:text-white hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="identityPage <= 1"
            @click="changeIdentityPage(identityPage - 1)"
          >
            {{ $t("settings.shared.prevPage") }}
          </button>
          <span class="text-sm font-medium text-white/80">{{
            $t("settings.shared.pageOf", { current: identityPage, total: identityTotalPages })
          }}</span>
          <button
            class="cursor-pointer rounded-lg border-none bg-[#e9ecef] px-4 py-1.5 text-sm font-medium text-[#495057] transition-all duration-200 hover:-translate-y-0.5 hover:bg-(--accent-color) hover:text-white hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="identityPage >= identityTotalPages"
            @click="changeIdentityPage(identityPage + 1)"
          >
            {{ $t("settings.shared.nextPage") }}
          </button>
        </div>

        <button class="btn-sm-primary" @click="openIdentityEditor(null)">
          {{ $t("settings.identity.create") }}
        </button>
      </div>
    </MenuItem>

    <!-- 转场描述弹窗：退场/出场方式都可留空；留空则该句描述旁白完全不生成，
         「离开了/加入了对话」与离场注记照常生成（只影响当前存档） -->
    <Teleport to="body">
      <div
        v-if="transitionTarget"
        class="fixed inset-0 z-[1200] flex items-center justify-center bg-black/50 backdrop-blur-sm"
      >
        <div
          class="w-[92vw] max-w-md rounded-2xl border border-white/10 bg-[#14141f]/95 p-4 text-white shadow-[0_8px_32px_rgba(0,0,0,0.45)]"
        >
          <h3 class="m-0 text-sm font-semibold tracking-wide">
            {{ $t("settings.identity.transition.title", { name: transitionTarget.name }) }}
          </h3>
          <p class="mt-1 text-[11px] leading-relaxed text-white/45">
            {{ $t("settings.identity.transition.hint") }}
          </p>

          <div class="mt-3 flex flex-col gap-1.5">
            <label class="text-xs font-medium text-white/60">{{
              $t("settings.identity.transition.exitLabel")
            }}</label>
            <input
              v-model="transitionExit"
              maxlength="200"
              class="identity-input"
              :placeholder="$t('settings.identity.transition.exitPlaceholder')"
            />
          </div>
          <div class="mt-3 flex flex-col gap-1.5">
            <label class="text-xs font-medium text-white/60">{{
              $t("settings.identity.transition.enterLabel")
            }}</label>
            <input
              v-model="transitionEnter"
              maxlength="200"
              class="identity-input"
              :placeholder="$t('settings.identity.transition.enterPlaceholder')"
            />
          </div>

          <div class="mt-4 flex justify-end gap-2">
            <button
              class="identity-btn"
              :disabled="transitionBusy"
              @click="transitionTarget = null"
            >
              {{ $t("settings.identity.transition.cancel") }}
            </button>
            <button
              class="identity-btn"
              :disabled="transitionBusy"
              @click="confirmSwitchIdentity(false)"
            >
              {{ $t("settings.identity.transition.skip") }}
            </button>
            <button
              class="identity-btn-primary"
              :disabled="transitionBusy"
              @click="confirmSwitchIdentity(true)"
            >
              {{ $t("settings.identity.transition.confirm") }}
            </button>
          </div>
        </div>
      </div>
    </Teleport>

    <RoleArchiveProgress />
    <NewCharacterModal
      :visible="newCharacterVisible"
      @close="newCharacterVisible = false"
      @created="handleCreated"
    />
    <PlayerIdentityModal
      :visible="identityEditorVisible"
      :identity-id="identityEditingId"
      :targets="relationTargets"
      @close="identityEditorVisible = false"
      @saved="onIdentitySaved"
    />
    <SettingsCharacterInfo
      :visible="createdEditorVisible"
      :role-id="createdCharacter?.character_id ?? null"
      :title="createdCharacter?.title"
      source="game"
      @close="closeCreatedEditor"
      @loaded="onGuideEditorLoaded"
      @saved="handleSettingsSaved"
    />

    <!-- 打开文件夹依赖桌面端文件管理器，移动端不可用，整卡隐藏 -->
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
            class="select-dark rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white transition-all duration-200 outline-none"
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
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Birdhouse, FolderOpen, PackageOpen, Plus, Rabbit, RefreshCcw, User } from "@lucide/vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import { invoke } from "@tauri-apps/api/core";

import CharacterCard from "../../ui/Menu/CharacterCard.vue";
import { Button } from "../../base";
import { MenuItem, MenuPage } from "../../ui";
import { characterGetAll, type CreatedCharacter } from "../../../api/services/character";
import type { RelationTargetOption } from "../../../composables/useRelationEditor";
import {
  listPlayerIdentities,
  setCurrentPlayerIdentity,
  startNewGameWithIdentity,
  aiKey,
  meKey,
  type PlayerIdentitySummary,
} from "../../../api/services/player-identity";
import { applyWebInitData } from "../../../stores/modules/game/actions";
import NewCharacterModal from "../character/NewCharacterModal.vue";
import PlayerIdentityModal from "../character/PlayerIdentityModal.vue";
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

const identities = ref<PlayerIdentitySummary[]>([]);
const identityLoading = ref(false);
const identityEditorVisible = ref(false);
const identityEditingId = ref<string | null>(null);

/** 关系目标：全部角色（不只当前分页）+ 我的身份（我侧） */
const relationTargets = computed<RelationTargetOption[]>(() => [
  ...allCharacters.value
    .filter((c) => !!c.resourceFolder)
    .map((c) => ({
      value: aiKey(c.resourceFolder as string),
      label: c.name,
      group: t("settings.identity.targetAi"),
    })),
  ...identities.value.map((i) => ({
    value: meKey(i.id),
    label: i.name,
    group: t("settings.identity.targetMe"),
  })),
]);

const IDENTITY_PAGE_SIZE = 5;
const identityPage = ref(1);

const identityTotalPages = computed(() =>
  Math.max(1, Math.ceil(identities.value.length / IDENTITY_PAGE_SIZE)),
);

const pagedIdentities = computed(() => {
  const start = (identityPage.value - 1) * IDENTITY_PAGE_SIZE;
  return identities.value.slice(start, start + IDENTITY_PAGE_SIZE);
});

const changeIdentityPage = (page: number) => {
  identityPage.value = Math.min(Math.max(page, 1), identityTotalPages.value);
};

/** 剧本进行中不允许换身份；权威判断在后端 guard，这里只是提前禁用按钮。 */
const identityScriptBlocked = computed(() => !!gameStore.runningScript);

const loadIdentities = async (): Promise<void> => {
  identityLoading.value = true;
  try {
    identities.value = await listPlayerIdentities();
    identityPage.value = Math.min(identityPage.value, identityTotalPages.value);
  } catch (e) {
    console.error("获取身份列表失败:", e);
    identities.value = [];
  } finally {
    identityLoading.value = false;
  }
};

const openIdentityEditor = (id: string | null) => {
  identityEditingId.value = id;
  identityEditorVisible.value = true;
};

const onIdentitySaved = async (id?: string) => {
  identityEditorVisible.value = false;
  await loadIdentities();
  // 新建的身份可能落在后面的分页，直接翻到它所在的页
  const index = id ? identities.value.findIndex((item) => item.id === id) : -1;
  if (index >= 0) changeIdentityPage(Math.floor(index / IDENTITY_PAGE_SIZE) + 1);
};

/** 换身份后半程的公共收尾：应用初始化数据、刷新身份列表、成功提示、在主菜单时回聊天页 */
const finishIdentityChange = async (
  gameInfo: Awaited<ReturnType<typeof setCurrentPlayerIdentity>>,
  title: string,
  message: string,
  exitStory = false,
) => {
  applyWebInitData(gameStore.$state, gameInfo);
  if (exitStory) gameStore.exitStoryMode();
  await loadIdentities();
  uiStore.showSuccess({ title, message });
  if (router.currentRoute.value.path === "/") {
    uiStore.showSettings = false;
    router.push("/chat");
  }
};

const startNewGame = async (item: PlayerIdentitySummary) => {
  const confirmed = await dialogStore.confirm(
    t("settings.identity.newGameConfirm", { name: item.name }),
  );
  if (!confirmed) return;
  try {
    const gameInfo = await startNewGameWithIdentity(item.id);
    await finishIdentityChange(
      gameInfo,
      t("settings.identity.newGameDoneTitle"),
      t("settings.identity.newGameDoneMsg", { name: item.name }),
      true,
    );
  } catch (e: any) {
    uiStore.showError({
      title: t("settings.identity.newGameFailTitle"),
      message: typeof e === "string" ? e : e.message || t("settings.identity.newGameFailTitle"),
    });
  }
};

/** 转场描述弹窗：确认切换后、执行前，让用户（可选地）描述退场/出场方式（都可留空） */
const transitionTarget = ref<PlayerIdentitySummary | null>(null);
const transitionExit = ref("");
const transitionEnter = ref("");
const transitionBusy = ref(false);

const switchIdentity = async (item: PlayerIdentitySummary) => {
  const confirmed = await dialogStore.confirm(
    t("settings.identity.switchConfirm", { name: item.name }),
  );
  if (!confirmed) return;
  transitionExit.value = "";
  transitionEnter.value = "";
  transitionTarget.value = item;
};

const confirmSwitchIdentity = async (withDesc: boolean) => {
  const item = transitionTarget.value;
  if (!item || transitionBusy.value) return;
  transitionBusy.value = true;
  try {
    const gameInfo = await setCurrentPlayerIdentity(
      item.id,
      withDesc ? transitionExit.value : "",
      withDesc ? transitionEnter.value : "",
    );
    transitionTarget.value = null;
    await finishIdentityChange(
      gameInfo,
      t("settings.identity.switchDoneTitle"),
      t("settings.identity.switchDoneMsg", { name: item.name }),
    );
  } catch (e: any) {
    uiStore.showError({
      title: t("settings.identity.switchFailTitle"),
      message: typeof e === "string" ? e : e.message || t("settings.identity.switchFailTitle"),
    });
  } finally {
    transitionBusy.value = false;
  }
};

onMounted(() => {
  loadCharacters();
  loadIdentities();
});

watch(
  () => gameStore.mainRoleId,
  () => {
    currentPage.value = 1;
    loadCharacters();
  },
);

// 换角色会按角色卡自动切换当前身份，列表的「当前使用」标记要跟着刷新
watch(
  () => gameStore.playerIdentityId,
  () => void loadIdentities(),
);

function scrollToRelationAnchor(id: string, attempts = 20): boolean {
  const el = document.getElementById(id);
  if (el) {
    el.scrollIntoView({ behavior: "smooth", block: "center" });
    return true;
  }
  if (attempts > 0) setTimeout(() => scrollToRelationAnchor(id, attempts - 1), 100);
  return false;
}

/** 关系引导要定位的区块；编辑器装载完成后消费，避免滚到空表单 */
const pendingGuideAnchor = ref<string | null>(null);

function tryScrollGuide() {
  const id = pendingGuideAnchor.value;
  if (!id) return;
  if (scrollToRelationAnchor(id)) pendingGuideAnchor.value = null;
}

function onGuideEditorLoaded() {
  void nextTick(tryScrollGuide);
}

function closeCreatedEditor() {
  createdEditorVisible.value = false;
  pendingGuideAnchor.value = null;
}

// 关系门「去填写」的意图：由聊天页写入，这里定位到对应编辑位置后清空
watch(
  () => uiStore.pendingRelationGuide,
  async (guide) => {
    if (!guide) return;
    if (guide.kind === "role") {
      // 首次进入设置页时角色列表可能还没回来，先等它，否则标题和目录名都拿不到
      if (!allCharacters.value.length) await loadCharacters();
      const target = allCharacters.value.find((c) => c.id === guide.roleId);
      createdCharacter.value = {
        character_id: guide.roleId,
        title: target?.title ?? gameStore.getGameRole(guide.roleId)?.roleName ?? "",
        resource_folder: target?.resourceFolder ?? "",
      };
      pendingGuideAnchor.value = "relation-block-role";
      createdEditorVisible.value = true;
      await nextTick();
      tryScrollGuide();
    } else {
      await loadIdentities();
      const identityId =
        gameStore.playerIdentityId ??
        identities.value.find((item) => item.is_current)?.id ??
        identities.value[0]?.id ??
        null;
      openIdentityEditor(identityId);
      pendingGuideAnchor.value = "relation-block-identity";
      await nextTick();
      tryScrollGuide();
    }
    uiStore.setPendingRelationGuide(null);
  },
  { immediate: true },
);
</script>
