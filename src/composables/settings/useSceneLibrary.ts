import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useGameStore } from "@/stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import {
  listScenes,
  createScene,
  updateScene,
  deleteScene,
  selectScene,
  type SceneInfo,
  type LightingParams,
} from "@/api/services/scene";
import { unlockAchievement } from "@/api/services/achievement";

/** 场景编辑弹窗的初始数据 / 提交数据 */
export interface SceneEditPayload {
  sceneName: string;
  sceneImage: string | null;
  sceneDescription: string;
  lighting?: LightingParams | null;
}

/**
 * 场景列表与增删改查、当前场景的选择。
 *
 * 只负责「数据」这一层：按分类过滤、分页、收藏排序等展示逻辑由调用方组合。
 */
export function useSceneLibrary() {
  const { t } = useI18n();
  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const dialogStore = useDialogStore();

  const scenes = ref<SceneInfo[]>([]);

  // 场景编辑弹窗
  const showSceneEdit = ref(false);
  const editMode = ref<"create" | "update">("create");
  const editingSceneId = ref<string | null>(null);
  const editInitialData = ref<SceneEditPayload | undefined>();

  const currentScene = computed(() => gameStore.currentScene);
  const currentSceneDisplay = computed(
    () => gameStore.currentScene?.scene_name || t("settings.background.scene.none"),
  );

  async function fetchScenes(): Promise<void> {
    try {
      scenes.value = await listScenes();
    } catch (error) {
      console.error("获取场景列表失败", error);
    }
  }

  function isSceneSelected(sceneId: string): boolean {
    return gameStore.currentScene?.id === sceneId;
  }

  /** 点击卡片：已激活则取消选中（背景转透明），否则选中并应用其背景 */
  async function handleSceneClick(scene: SceneInfo): Promise<void> {
    if (gameStore.currentScene?.id === scene.id) {
      gameStore.clearCurrentScene();
      uiStore.setCurrentBackground("");
      unlockAchievement("see_through").catch(console.error);
      await fetchScenes();
      return;
    }

    // 无描述时提醒用户
    if (!scene.scene_description?.trim()) {
      uiStore.showInfo({
        title: t("settings.background.scene.tip"),
        message: t("settings.background.scene.noDescriptionTip", { name: scene.scene_name }),
        duration: 4000,
      });
    }

    try {
      await selectScene(scene.id);
      gameStore.setCurrentScene(scene);
      if (scene.background) {
        uiStore.setCurrentBackground(scene.background);
      }
      await fetchScenes();
    } catch (error) {
      console.error("选择场景失败", error);
    }
  }

  function openCreateScene(): void {
    editMode.value = "create";
    editingSceneId.value = null;
    editInitialData.value = undefined;
    showSceneEdit.value = true;
  }

  function openEditScene(scene: SceneInfo): void {
    editMode.value = "update";
    editingSceneId.value = scene.id;
    editInitialData.value = {
      sceneName: scene.scene_name,
      sceneImage: scene.background || null,
      sceneDescription: scene.scene_description,
      lighting: scene.lighting,
    };
    showSceneEdit.value = true;
  }

  async function submitScene(data: SceneEditPayload): Promise<void> {
    try {
      const payload = {
        scene_name: data.sceneName,
        scene_description: data.sceneDescription,
        background: data.sceneImage || "",
        lighting: data.lighting ?? null,
      };
      if (editMode.value === "create") {
        await createScene(payload);
      } else {
        if (!editingSceneId.value) return;
        await updateScene({ id: editingSceneId.value, ...payload });
      }
      showSceneEdit.value = false;
      await fetchScenes();

      // 如果更新的是当前选中的场景，立即同步到 gameStore 使光影等参数即时生效
      if (editMode.value === "update" && editingSceneId.value === gameStore.currentScene?.id) {
        const updatedScene = scenes.value.find((s) => s.id === editingSceneId.value);
        if (updatedScene) {
          gameStore.setCurrentScene(updatedScene);
          if (updatedScene.background) {
            uiStore.setCurrentBackground(updatedScene.background);
          }
        }
      }
    } catch (error) {
      console.error("操作失败", error);
    }
  }

  /** 删除当前选中的场景；插件场景只读，不可删除 */
  async function deleteCurrentScene(): Promise<void> {
    const scene = currentScene.value;
    if (!scene) return;
    if (scene.source && scene.source !== "game") {
      await dialogStore.alert(t("settings.background.scene.pluginNotDeletable"));
      return;
    }
    if (
      !(await dialogStore.confirm(
        t("settings.background.scene.deleteConfirm", { name: scene.scene_name }),
      ))
    )
      return;

    try {
      await deleteScene(scene.id);
      gameStore.clearCurrentScene();
      await fetchScenes();
    } catch (error) {
      console.error("删除场景失败", error);
    }
  }

  return {
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
  };
}
