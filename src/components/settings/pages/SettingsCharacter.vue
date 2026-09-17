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
          @saved="handleSettingsSaved"
        />
      </div>

      <div v-if="totalPages > 1" class="flex w-full items-center justify-between px-3 py-2">
        <button
          class="cursor-pointer rounded-lg border-none bg-[#e9ecef] px-4 py-1.5 text-sm font-medium
            text-[#495057] transition-all duration-200 hover:-translate-y-0.5
            hover:bg-(--accent-color) hover:text-white
            hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:cursor-not-allowed
            disabled:opacity-40"
          :disabled="currentPage <= 1"
          @click="changePage(currentPage - 1)"
        >
          {{ $t("settings.shared.prevPage") }}
        </button>
        <span class="text-sm font-medium text-white/80">{{
          $t("settings.shared.pageOf", { current: currentPage, total: totalPages })
        }}</span>
        <button
          class="cursor-pointer rounded-lg border-none bg-[#e9ecef] px-4 py-1.5 text-sm font-medium
            text-[#495057] transition-all duration-200 hover:-translate-y-0.5
            hover:bg-(--accent-color) hover:text-white
            hover:shadow-[0_4px_10px_rgba(121,217,255,0.4)] disabled:cursor-not-allowed
            disabled:opacity-40"
          :disabled="currentPage >= totalPages"
          @click="changePage(currentPage + 1)"
        >
          {{ $t("settings.shared.nextPage") }}
        </button>
      </div>
    </MenuItem>

    <!-- ── 我的身份 ──────────────────────────────────────────────
         决定「我」是谁：名字 / 副标题 / 注入聊天的身份提示词 / 与各角色的关系。
         后端限制：剧本进行中不能更换身份（会返回错误提示）。 -->
    <MenuItem :title="$t('settings.identity.title')">
      <template #header>
        <User :size="20" />
      </template>

      <div class="space-y-3">
        <p class="text-xs leading-relaxed text-white/50">{{ $t("settings.identity.hint") }}</p>

        <div v-if="identityLoading" class="text-sm text-white/50">
          {{ $t("settings.shared.loading") }}
        </div>

        <p v-else-if="identities.length === 0" class="text-sm text-white/50">
          {{ $t("settings.identity.empty") }}
        </p>

        <div v-else class="flex flex-col gap-2">
          <div
            v-for="item in identities"
            :key="item.id"
            class="flex items-start justify-between gap-3 rounded-xl border border-white/10
              bg-black/20 px-3 py-2"
          >
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <span class="truncate text-sm font-medium text-white">{{ item.name }}</span>
                <span v-if="item.subtitle" class="truncate text-xs text-white/50">{{
                  item.subtitle
                }}</span>
                <span
                  v-if="item.is_current"
                  class="shrink-0 rounded-full bg-[#79d9ff] px-2 py-0.5 text-[10px] text-white"
                  >{{ $t("settings.identity.current") }}</span
                >
              </div>
              <p v-if="item.prompt" class="mt-0.5 line-clamp-2 text-xs text-white/40">
                {{ item.prompt }}
              </p>
              <p v-if="item.relation_count > 0" class="mt-0.5 text-[11px] text-white/35">
                {{ $t("settings.identity.relationCount", { n: item.relation_count }) }}
              </p>
            </div>

            <div class="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
              <button v-if="!item.is_current" class="identity-btn" @click="useIdentity(item.id)">
                {{ $t("settings.identity.use") }}
              </button>
              <button class="identity-btn" @click="editIdentity(item.id)">
                {{ $t("settings.identity.edit") }}
              </button>
              <button class="identity-btn" @click="removeIdentity(item)">
                {{ $t("settings.identity.delete") }}
              </button>
            </div>
          </div>
        </div>

        <button class="identity-btn-primary" @click="startCreateIdentity">
          {{ $t("settings.identity.create") }}
        </button>

        <!-- 内联编辑表单 -->
        <div v-if="identityForm" class="space-y-3 rounded-xl border border-white/10 bg-black/20 p-3">
          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-white/60">{{
              $t("settings.identity.fieldName")
            }}</label>
            <input v-model="identityForm.name" class="identity-input" />
          </div>
          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-white/60">{{
              $t("settings.identity.fieldSubtitle")
            }}</label>
            <input v-model="identityForm.subtitle" class="identity-input" />
          </div>
          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-white/60">{{
              $t("settings.identity.fieldPrompt")
            }}</label>
            <textarea v-model="identityForm.prompt" rows="4" class="identity-input"></textarea>
          </div>

          <!-- 关系：我和各个 AI 角色 / 我的其他身份 -->
          <div class="flex flex-col gap-2">
            <label class="text-xs font-medium text-white/60">{{
              $t("settings.identity.fieldRelations")
            }}</label>
            <p class="text-[11px] leading-relaxed text-white/40">
              {{ $t("settings.identity.relationsHint") }}
            </p>
            <div v-for="(row, idx) in relationRows" :key="idx" class="flex items-center gap-2">
              <select v-model="row.target" class="identity-input w-40 shrink-0">
                <optgroup :label="$t('settings.identity.targetAi')">
                  <option v-for="r in relationTargetsAi" :key="r.value" :value="r.value">
                    {{ r.label }}
                  </option>
                </optgroup>
                <optgroup :label="$t('settings.identity.targetMe')">
                  <option v-for="r in relationTargetsMe" :key="r.value" :value="r.value">
                    {{ r.label }}
                  </option>
                </optgroup>
              </select>
              <input v-model="row.text" class="identity-input flex-1" />
              <button class="identity-btn" @click="removeRelationRow(idx)">×</button>
            </div>
            <button class="identity-btn" @click="addRelationRow">
              {{ $t("settings.identity.addRelation") }}
            </button>
          </div>

          <div class="flex justify-end gap-2 pt-1">
            <button class="identity-btn" :disabled="savingIdentity" @click="identityForm = null">
              {{ $t("settings.identity.cancel") }}
            </button>
            <button class="identity-btn-primary" :disabled="savingIdentity" @click="submitIdentity">
              {{ savingIdentity ? $t("settings.shared.loading") : $t("settings.identity.save") }}
            </button>
          </div>
        </div>
      </div>
    </MenuItem>

    <RoleArchiveProgress />

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
            class="rounded-xl border border-white/10 bg-black/20 px-3 py-2 text-sm text-white
              transition-all duration-200 outline-none"
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
  import { computed, onMounted, ref, watch } from "vue";
  import { useI18n } from "vue-i18n";
  import { useRouter } from "vue-router";
  import { Birdhouse, FolderOpen, PackageOpen, Rabbit, RefreshCcw, User } from "lucide-vue-next";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { invoke } from "@tauri-apps/api/core";

  import CharacterCard from "../../ui/Menu/CharacterCard.vue";
  import { Button } from "../../base";
  import { MenuItem, MenuPage } from "../../ui";
  import { characterGetAll } from "../../../api/services/character";
  import {
    deletePlayerIdentity,
    emptyPlayerIdentity,
    getPlayerIdentity,
    listPlayerIdentities,
    savePlayerIdentity,
    setCurrentPlayerIdentity,
    aiKey,
    meKey,
    type PlayerIdentity,
    type PlayerIdentitySummary,
  } from "../../../api/services/player-identity";
  import { applyWebInitData } from "../../../stores/modules/game/actions";
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
  const currentPage = ref(1);
  const totalPages = ref(1);
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

  const fetchCharacters = async (page: number): Promise<void> => {
    try {
      const result = await characterGetAll(page);
      totalPages.value = result.total_pages;

      // 防御：删除角色后当前页可能超出 total_pages（例如停在第 2 页删掉最后一个），
      // 此时分页条 v-if="totalPages > 1" 整条消失，回退按钮不存在 → 空白列表死锁。
      // 钳制并重取最后一页。
      if (currentPage.value > result.total_pages && result.total_pages > 0) {
        currentPage.value = result.total_pages;
        await fetchCharactersInternal(result.total_pages);
        return;
      }

      characters.value = result.items.map(mapCharacter);
    } catch (error) {
      console.error("获取角色列表失败:", error);
      characters.value = [];
    }
  };

  // fetchCharacters 的内部调用（钳制回退时用，避免无限递归）
  const fetchCharactersInternal = async (page: number): Promise<void> => {
    try {
      const result = await characterGetAll(page);
      totalPages.value = result.total_pages;
      characters.value = result.items.map(mapCharacter);
    } catch (error) {
      console.error("获取角色列表失败:", error);
      characters.value = [];
    }
  };

  const loadCharacters = async (): Promise<void> => {
    await fetchCharacters(currentPage.value);
  };

  const changePage = async (page: number): Promise<void> => {
    currentPage.value = page;
    await fetchCharacters(page);
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

  // ── 我的身份 ────────────────────────────────────────────────
  //
  // 身份卡决定「我」是谁；后端把「当前身份」与「该角色对我的称呼」分成两个出口，
  // 所以这里改的只是身份卡，不会动到 AI 角色卡上的 user_name。
  const identities = ref<PlayerIdentitySummary[]>([]);
  const identityLoading = ref(false);
  const savingIdentity = ref(false);
  const identityForm = ref<PlayerIdentity | null>(null);
  /** 编辑表单里的关系行（目标下拉 + 文本）；保存时折成 relations 映射 */
  const relationRows = ref<{ target: string; text: string }[]>([]);

  const relationTargetsAi = computed(() =>
    characters.value
      .filter((c) => !!c.resourceFolder)
      .map((c) => ({ value: aiKey(c.resourceFolder as string), label: c.name }))
  );

  const relationTargetsMe = computed(() =>
    identities.value
      .filter((i) => i.id !== identityForm.value?.id)
      .map((i) => ({ value: meKey(i.id), label: i.name }))
  );

  const loadIdentities = async (): Promise<void> => {
    identityLoading.value = true;
    try {
      identities.value = await listPlayerIdentities();
    } catch (e) {
      console.error("获取身份列表失败:", e);
      identities.value = [];
    } finally {
      identityLoading.value = false;
    }
  };

  const syncRelationRows = (identity: PlayerIdentity) => {
    relationRows.value = Object.entries(identity.relations || {})
      .filter(([, text]) => !!text)
      .map(([target, text]) => ({ target, text }));
  };

  const startCreateIdentity = () => {
    const form = emptyPlayerIdentity();
    identityForm.value = form;
    syncRelationRows(form);
  };

  const editIdentity = async (id: string) => {
    try {
      const identity = await getPlayerIdentity(id);
      if (!identity) return;
      identityForm.value = identity;
      syncRelationRows(identity);
    } catch (e) {
      console.error("读取身份失败:", e);
    }
  };

  const addRelationRow = () => {
    relationRows.value.push({ target: relationTargetsAi.value[0]?.value ?? "", text: "" });
  };

  const removeRelationRow = (idx: number) => {
    relationRows.value.splice(idx, 1);
  };

  const submitIdentity = async () => {
    const form = identityForm.value;
    if (!form) return;
    if (!form.name.trim()) {
      uiStore.showError({
        title: t("settings.identity.msg.saveFailTitle"),
        message: t("settings.identity.msg.nameRequired"),
      });
      return;
    }

    // 关系行折回映射：空目标 / 空文本直接丢弃（后端也会再过滤一次）
    const relations: Record<string, string> = {};
    for (const row of relationRows.value) {
      if (row.target && row.text.trim()) relations[row.target] = row.text.trim();
    }

    savingIdentity.value = true;
    try {
      await savePlayerIdentity({ ...form, relations });
      identityForm.value = null;
      await loadIdentities();
      uiStore.showSuccess({
        title: t("settings.identity.msg.savedTitle"),
        message: t("settings.identity.msg.savedMsg"),
      });
    } catch (e: any) {
      uiStore.showError({
        title: t("settings.identity.msg.saveFailTitle"),
        message: typeof e === "string" ? e : e.message || t("settings.identity.msg.saveFailTitle"),
      });
    } finally {
      savingIdentity.value = false;
    }
  };

  /** 切换当前身份。后端在剧本进行中会拒绝（见 player_identity::guard）。 */
  const useIdentity = async (id: string) => {
    try {
      const gameInfo = await setCurrentPlayerIdentity(id);
      applyWebInitData(gameStore.$state, gameInfo);
      await loadIdentities();
    } catch (e: any) {
      uiStore.showError({
        title: t("settings.identity.msg.switchFailTitle"),
        message:
          typeof e === "string" ? e : e.message || t("settings.identity.msg.switchFailTitle"),
      });
    }
  };

  const removeIdentity = async (item: PlayerIdentitySummary) => {
    const confirmed = await dialogStore.confirm(
      t("settings.identity.msg.deleteConfirm", { name: item.name })
    );
    if (!confirmed) return;
    try {
      await deletePlayerIdentity(item.id);
      await loadIdentities();
    } catch (e: any) {
      uiStore.showError({
        title: t("settings.identity.msg.deleteFailTitle"),
        message: typeof e === "string" ? e : e.message || t("settings.identity.msg.deleteFailTitle"),
      });
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
    }
  );
</script>

<style scoped>
  /* 「我的身份」区块的小控件样式。
     刻意用原生元素 + 纯 CSS，避免依赖 base 组件库的具体 props。 */
  .identity-input {
    border-radius: 0.75rem;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(0, 0, 0, 0.2);
    padding: 0.5rem 0.75rem;
    font-size: 0.875rem;
    color: #fff;
    outline: none;
  }

  .identity-btn {
    cursor: pointer;
    border: none;
    border-radius: 0.5rem;
    background: #e9ecef;
    padding: 0.25rem 0.6rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: #495057;
    transition: all 0.2s;
  }

  .identity-btn:hover:not(:disabled) {
    background: var(--accent-color, #79d9ff);
    color: #fff;
  }

  .identity-btn-primary {
    cursor: pointer;
    border: none;
    border-radius: 0.5rem;
    background: var(--accent-color, #79d9ff);
    padding: 0.4rem 0.9rem;
    font-size: 0.8rem;
    font-weight: 500;
    color: #fff;
    transition: all 0.2s;
  }

  .identity-btn-primary:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  .identity-btn:disabled,
  .identity-btn-primary:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }
</style>
