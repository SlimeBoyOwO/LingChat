<template>
  <!-- ========== 场景管理 ========== -->
  <MenuItem :title="$t('settings.background.scene.title')">
    <template #header>
      <PictureInPicture :size="20" />
    </template>

    <!-- 当前场景信息 + 操作按钮 -->
    <div class="mb-4 flex flex-wrap items-center gap-3">
      <div class="text-brand font-bold">
        {{ $t("settings.background.scene.current") }}{{ currentSceneDisplay }}
      </div>
      <div class="ml-auto flex flex-wrap gap-2">
        <button
          class="bg-brand/80 border-brand hover:bg-brand rounded-full border px-5 py-1.5 text-sm font-bold text-white shadow-lg shadow-indigo-500/20 transition-all"
          @click="openCreateScene"
        >
          {{ $t("settings.background.scene.create") }}
        </button>
        <button
          class="rounded-full border border-white/20 bg-white/10 px-4 py-1.5 text-sm font-bold text-white/80 shadow-lg transition-all hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="isBackgroundCategoryReadOnly"
          @click="triggerUpload"
        >
          {{ $t("settings.background.scene.upload") }}
        </button>
        <button
          v-if="!isAndroid()"
          class="rounded-full border border-white/20 bg-white/10 px-4 py-1.5 text-sm font-bold text-white/80 shadow-lg transition-all hover:bg-white/20"
          @click="handleOpenFolder"
        >
          {{ $t("settings.background.scene.openFolder") }}
        </button>
        <button
          class="rounded-full border border-white/20 bg-white/10 px-4 py-1.5 text-sm font-bold text-white/80 shadow-lg transition-all hover:bg-white/20"
          @click="openSortModal"
        >
          {{ $t("settings.background.sort.button") }}
        </button>
        <button
          class="rounded-full border border-white/20 bg-white/10 px-4 py-1.5 text-sm font-bold text-white/80 shadow-lg transition-all hover:bg-white/20"
          @click="handleRefreshScenes"
          :title="$t('settings.background.scene.refreshTip')"
        >
          {{ $t("settings.background.scene.refresh") }}
        </button>
        <button
          class="rounded-full border border-red-500/30 bg-red-500/20 px-4 py-1.5 text-sm font-bold text-red-300 shadow-lg transition-all hover:bg-red-500/30"
          :disabled="!currentScene"
          @click="deleteCurrentScene"
        >
          {{ $t("settings.background.scene.delete") }}
        </button>
      </div>
    </div>

    <SceneCategoryBar
      v-model="currentBackgroundCategory"
      v-model:name="newCategoryName"
      :categories="backgroundCategories"
      :busy="categoryBusy"
      @create="createCategory"
      @delete="deleteCurrentCategory"
    />

    <div
      v-if="filteredScenes.length === 0"
      class="mb-5 rounded-xl border border-dashed border-white/15 bg-white/5 px-6 py-10 text-center text-sm text-white/50"
    >
      {{ $t("settings.background.scene.categoryEmpty") }}
    </div>

    <!-- 场景卡片网格 -->
    <div class="grid w-full grid-cols-1 gap-5 pb-5 sm:grid-cols-2 xl:grid-cols-3">
      <SceneCard
        v-for="scene in paginatedScenes"
        :key="scene.id"
        :scene="scene"
        :selected="isSceneSelected(scene.id)"
        :favored="isFavored(scene.id)"
        @select="handleSceneClick"
        @edit="openEditScene"
        @toggle-favorite="handleToggleFavorite"
        @context-menu="openSceneContextMenu"
      />
    </div>

    <SceneContextMenu
      :visible="sceneMenu.visible"
      :x="sceneMenu.x"
      :y="sceneMenu.y"
      :scene="sceneMenu.scene"
      :categories="writableBackgroundCategories"
      @close="closeSceneContextMenu"
      @move="handleMoveScene"
    />

    <ScenePagination v-model="currentPage" :total-pages="totalPages" />

    <!-- 隐藏的文件上传 input -->
    <input
      type="file"
      ref="uploadInput"
      @change="handleFileUpload"
      :accept="ALLOWED_BACKGROUND_EXTENSIONS.join(',')"
      style="display: none"
    />
  </MenuItem>

  <SceneSortModal
    :show="showSortModal"
    :items="sortItems"
    :favored-ids="sortFavoredIds"
    @close="showSortModal = false"
    @save="handleSaveSortOrder"
  />

  <SceneEditModal
    :show="showSceneEdit"
    :mode="editMode"
    :backgrounds="backgroundList"
    :initial-data="editInitialData"
    @close="showSceneEdit = false"
    @submit="submitScene"
    @upload="triggerUpload"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { PictureInPicture } from "lucide-vue-next";
import { MenuItem } from "@/components/ui";
import SceneEditModal from "@/components/settings/scene/SceneEditModal.vue";
import { isAndroid } from "@/utils/platform";
import { clearEmptyScenes, moveSceneToCategory, type SceneInfo } from "@/api/services/scene";
import { openBackgroundsFolder } from "@/api/services/background";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import { useSceneLibrary } from "@/composables/settings/useSceneLibrary";
import { useSceneFavorites } from "@/composables/settings/useSceneFavorites";
import {
  ALLOWED_BACKGROUND_EXTENSIONS,
  useBackgroundLibrary,
} from "@/composables/settings/useBackgroundLibrary";
import { ALL_CATEGORY, ROOT_CATEGORY, VIRTUAL_CATEGORY } from "@/constants/background-categories";
import SceneCard from "./SceneCard.vue";
import SceneCategoryBar from "./SceneCategoryBar.vue";
import SceneContextMenu from "./SceneContextMenu.vue";
import ScenePagination from "./ScenePagination.vue";
import SceneSortModal from "./SceneSortModal.vue";

const { t } = useI18n();
const gameStore = useGameStore();
const uiStore = useUIStore();
const dialogStore = useDialogStore();

const {
  scenes,
  showSceneEdit,
  editMode,
  editInitialData,
  currentScene,
  currentSceneDisplay,
  fetchScenes,
  isSceneSelected,
  handleSceneClick,
  openCreateScene,
  openEditScene,
  submitScene,
  deleteCurrentScene,
} = useSceneLibrary();

const { applySceneOrder, isFavored, toggleFavorite, loadFavored, saveOrder } = useSceneFavorites();

const {
  backgroundList,
  backgroundCategories,
  currentBackgroundCategory,
  newCategoryName,
  isBackgroundCategoryReadOnly,
  writableBackgroundCategories,
  categoryBusy,
  refreshBackground,
  createCategory,
  deleteCurrentCategory,
  uploadBackground,
} = useBackgroundLibrary({ onChanged: fetchScenes });

// ── 场景过滤 + 分页 ──
// 收藏置顶排序后的完整场景列表（再按背景分类过滤）
const orderedScenes = computed(() => applySceneOrder(scenes.value));

const filteredScenes = computed(() => {
  const category = currentBackgroundCategory.value;
  if (!category || category === ALL_CATEGORY) return orderedScenes.value;
  if (category === VIRTUAL_CATEGORY) {
    return orderedScenes.value.filter(
      (scene) => scene.plugin_id || (scene.source && scene.source !== "game"),
    );
  }
  return orderedScenes.value.filter((s) => {
    if (s.plugin_id || (s.source && s.source !== "game")) return false;
    return (s.category ?? ROOT_CATEGORY) === category;
  });
});

const ITEMS_PER_PAGE = 6;
const currentPage = ref(1);
const totalPages = computed(() =>
  Math.max(1, Math.ceil(filteredScenes.value.length / ITEMS_PER_PAGE)),
);
const paginatedScenes = computed(() => {
  const start = (currentPage.value - 1) * ITEMS_PER_PAGE;
  return filteredScenes.value.slice(start, start + ITEMS_PER_PAGE);
});
// 场景或切分类变化时回到第一页
watch([scenes, currentBackgroundCategory], () => {
  if (currentPage.value > totalPages.value) currentPage.value = totalPages.value;
});

// ── 收藏置顶 ──
function handleToggleFavorite(scene: SceneInfo): void {
  toggleFavorite(scene.id);
  scenes.value = applySceneOrder(scenes.value);
}

// ── 排序面板 ──
const showSortModal = ref(false);
const sortItems = ref<SceneInfo[]>([]);
const sortFavoredIds = ref<string[]>([]);

async function openSortModal(): Promise<void> {
  // 全局收藏与排序键是跨分类的，在子分类下保存会用当前子集覆盖全局，
  // 因此只允许在「全部」分类下调整全局排序。
  if (currentBackgroundCategory.value !== ALL_CATEGORY) {
    await dialogStore.alert(t("settings.background.sort.onlyAll"));
    return;
  }
  sortItems.value = filteredScenes.value.map((s) => ({ ...s }));
  sortFavoredIds.value = loadFavored();
  showSortModal.value = true;
}

function handleSaveSortOrder(ordered: SceneInfo[]): void {
  const favoredSet = new Set(loadFavored());
  saveOrder(
    ordered.filter((s) => favoredSet.has(s.id)).map((s) => s.id),
    ordered.filter((s) => !favoredSet.has(s.id)).map((s) => s.id),
  );
  showSortModal.value = false;
  scenes.value = applySceneOrder(scenes.value);
  uiStore.showSuccess({
    title: t("settings.background.sort.saved"),
    duration: 2000,
  });
}

// ── 上传背景 ──
const uploadInput = ref<HTMLInputElement | null>(null);

function triggerUpload(): void {
  if (isBackgroundCategoryReadOnly.value) return;
  uploadInput.value?.click();
}

async function handleFileUpload(event: Event): Promise<void> {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;
  await uploadBackground(file);
  target.value = "";
}

// ── 右键菜单：移动到子分类 ──
const sceneMenu = ref<{
  visible: boolean;
  x: number;
  y: number;
  scene: SceneInfo | null;
}>({ visible: false, x: 0, y: 0, scene: null });

function openSceneContextMenu(scene: SceneInfo, event: MouseEvent): void {
  if (scene.source && scene.source !== "game") return;
  sceneMenu.value = { visible: true, x: event.clientX, y: event.clientY, scene };
}

function closeSceneContextMenu(): void {
  sceneMenu.value.visible = false;
}

/** 把场景的背景图片移动到目标子分类（子文件夹） */
async function handleMoveScene(category: string | null): Promise<void> {
  const scene = sceneMenu.value.scene;
  closeSceneContextMenu();
  if (!scene) return;
  if (!scene.background) {
    await dialogStore.alert(t("settings.background.scene.moveNoBackground"));
    return;
  }
  try {
    await moveSceneToCategory(scene.id, category);
    await refreshBackground();
    await fetchScenes();
    uiStore.showSuccess({
      title: t("settings.background.scene.movedTitle"),
      message: t("settings.background.scene.movedMsg", {
        name: scene.scene_name,
        category: category ?? t("settings.background.scene.moveToRoot"),
      }),
      duration: 3000,
    });
  } catch (e: any) {
    console.error("移动场景到分类失败:", e);
    uiStore.showError({
      title: t("settings.background.scene.moveFail"),
      message: e?.message || "",
    });
  }
}

// ── 刷新：删除空场景 + 刷新背景 + 刷新列表（合并为一个按钮）──
async function handleRefreshScenes(): Promise<void> {
  const confirmed = await dialogStore.confirm(t("settings.background.scene.refreshConfirm"));
  if (!confirmed) return;
  try {
    await clearEmptyScenes();
    await refreshBackground();
    await fetchScenes();
    uiStore.showSuccess({
      title: t("settings.background.scene.refreshDone"),
      message: t("settings.background.scene.refreshDoneMsg"),
      duration: 3000,
    });
  } catch (e: any) {
    console.error("刷新场景失败:", e);
    uiStore.showError({
      title: t("settings.background.scene.refreshFail"),
      message: e?.message || "",
    });
  }
}

async function handleOpenFolder(): Promise<void> {
  try {
    await openBackgroundsFolder();
  } catch (e: any) {
    uiStore.showError({
      title: t("settings.background.folder.errorTitle"),
      message: t("settings.background.folder.openFailed"),
    });
  }
}

onMounted(async () => {
  try {
    await refreshBackground();
  } catch (error) {
    console.error("加载背景图片失败", error);
  }

  await fetchScenes();

  // 恢复上次选中的场景
  if (gameStore.currentScene?.background) {
    uiStore.setCurrentBackground(gameStore.currentScene.background);
  }
});
</script>
