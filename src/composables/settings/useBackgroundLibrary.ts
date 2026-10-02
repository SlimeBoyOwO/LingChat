import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import {
  getBackgroundImages,
  uploadBackgroundImage,
  listBackgroundCategories,
  createBackgroundCategory,
  deleteBackgroundCategory,
} from "@/api/services/background";
import type { BackgroundImageInfo } from "@/types";

import { ALL_CATEGORY, ROOT_CATEGORY, VIRTUAL_CATEGORY } from "@/constants/background-categories";

/** 允许上传的背景图片扩展名 */
export const ALLOWED_BACKGROUND_EXTENSIONS = [
  ".jpg",
  ".jpeg",
  ".png",
  ".webp",
  ".bmp",
  ".svg",
  ".tif",
  ".gif",
];

interface UseBackgroundLibraryOptions {
  /** 背景库发生变化（新建/删除分类、上传）后的回调，用于同步场景列表 */
  onChanged?: () => Promise<void> | void;
}

/**
 * 背景图片库：图片列表、子分类（= 子文件夹）的读写与上传。
 */
export function useBackgroundLibrary(options: UseBackgroundLibraryOptions = {}) {
  const { t } = useI18n();
  const uiStore = useUIStore();
  const dialogStore = useDialogStore();

  const backgroundList = ref<BackgroundImageInfo[]>([]);
  const backgroundCategories = ref<string[]>([]);
  const currentBackgroundCategory = ref<string>(ALL_CATEGORY);
  /** 「新建分类」输入框的内容 */
  const newCategoryName = ref("");
  const categoryBusy = ref(false);

  const isBackgroundCategoryReadOnly = computed(
    () => currentBackgroundCategory.value === VIRTUAL_CATEGORY,
  );
  const writableBackgroundCategories = computed(() =>
    backgroundCategories.value.filter(
      (category) => category !== VIRTUAL_CATEGORY && category !== ROOT_CATEGORY,
    ),
  );

  async function fetchBackgrounds(): Promise<BackgroundImageInfo[]> {
    try {
      const data = await getBackgroundImages();
      return data.map((background: BackgroundImageInfo) => ({
        title: background.title || "Untitled",
        url: background.url || "",
        time: background.time,
        // 保留所属子分类（子文件夹名），否则按分类选项卡过滤场景时会全部落到“根目录”
        category: background.category,
      }));
    } catch (error) {
      console.error("Failed to fetch background list:", error);
      return [];
    }
  }

  async function loadBackgroundCategories(): Promise<void> {
    try {
      const cats = await listBackgroundCategories();
      backgroundCategories.value = [ROOT_CATEGORY, ...cats.filter((cat) => cat !== ROOT_CATEGORY)];
      if (
        currentBackgroundCategory.value !== ALL_CATEGORY &&
        !backgroundCategories.value.includes(currentBackgroundCategory.value)
      ) {
        currentBackgroundCategory.value = ALL_CATEGORY;
      }
    } catch (error) {
      console.error("加载背景分类失败", error);
    }
  }

  /** 重新拉取背景图片与分类；本地增删后统一走这里，保证两侧数据一致 */
  async function refreshBackground(): Promise<void> {
    backgroundList.value = await fetchBackgrounds();
    await loadBackgroundCategories();
  }

  async function refreshAndNotifyScenes(): Promise<void> {
    await refreshBackground();
    await options.onChanged?.();
  }

  /** 用输入框中的名字新建分类；成功返回 true（失败时已提示用户） */
  async function createCategory(): Promise<boolean> {
    if (categoryBusy.value) return false;
    const name = newCategoryName.value.trim();
    if (!name) {
      await dialogStore.alert(t("settings.background.scene.categoryNameEmpty"));
      return false;
    }
    categoryBusy.value = true;
    try {
      await createBackgroundCategory(name);
      newCategoryName.value = "";
      await refreshAndNotifyScenes();
      currentBackgroundCategory.value = name;
      uiStore.showSuccess({
        title: t("settings.background.scene.categoryCreated"),
        message: t("settings.background.scene.categoryCreatedMsg", { name }),
        duration: 3000,
      });
      return true;
    } catch (error: any) {
      console.error("创建分类失败:", error);
      await dialogStore.alert(
        `${t("settings.background.scene.categoryCreateFail")}\n${String(error)}`,
      );
      return false;
    } finally {
      categoryBusy.value = false;
    }
  }

  /** 删除当前分类：其中的背景移回根目录，场景不会被删除 */
  async function deleteCurrentCategory(): Promise<boolean> {
    const category = currentBackgroundCategory.value;
    if (
      categoryBusy.value ||
      !category ||
      category === ALL_CATEGORY ||
      category === ROOT_CATEGORY ||
      category === VIRTUAL_CATEGORY
    )
      return false;
    categoryBusy.value = true;
    try {
      const confirmed = await dialogStore.confirm(
        t("settings.background.scene.categoryDeleteConfirmMove", { name: category }),
      );
      if (!confirmed) return false;
      const movedCount = await deleteBackgroundCategory(category, "move_to_root");
      currentBackgroundCategory.value = ALL_CATEGORY;
      await refreshAndNotifyScenes();
      uiStore.showSuccess({
        title: t("settings.background.scene.categoryDeleted"),
        message: t("settings.background.scene.categoryDeletedMoved", {
          name: category,
          count: movedCount,
        }),
        duration: 3000,
      });
      return true;
    } catch (error) {
      console.error("删除分类失败:", error);
      await dialogStore.alert(
        `${t("settings.background.scene.categoryDeleteFail")}\n${String(error)}`,
      );
      return false;
    } finally {
      categoryBusy.value = false;
    }
  }

  /**
   * 上传背景图片到当前分类（「全部」表示根目录）。
   * 后端会自动把新背景注册为场景，因此成功后同样需要刷新场景列表。
   */
  async function uploadBackground(file: File): Promise<boolean> {
    if (isBackgroundCategoryReadOnly.value) return false;

    const fileExt = file.name.slice(file.name.lastIndexOf(".")).toLowerCase();
    if (!ALLOWED_BACKGROUND_EXTENSIONS.includes(fileExt)) {
      await dialogStore.alert(
        t("settings.background.upload.invalidFormat", {
          formats: ALLOWED_BACKGROUND_EXTENSIONS.join(", "),
        }),
      );
      return false;
    }

    try {
      const buf = await file.arrayBuffer();
      const category =
        currentBackgroundCategory.value === ALL_CATEGORY
          ? undefined
          : currentBackgroundCategory.value;
      await uploadBackgroundImage(file.name, new Uint8Array(buf), category);
      await refreshAndNotifyScenes();
      return true;
    } catch (error) {
      console.error("上传失败", error);
      await dialogStore.alert(t("settings.background.upload.failed"));
      return false;
    }
  }

  return {
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
  };
}
