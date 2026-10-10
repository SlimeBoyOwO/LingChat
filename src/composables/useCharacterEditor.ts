import { inject, type InjectionKey, type Ref } from "vue";
import * as api from "@/api/services/character";

export interface CharacterEditorContext {
  editId: Ref<string | undefined>;
  busy: Ref<number>;
  revision: Ref<number>;
}
export const characterEditorKey: InjectionKey<CharacterEditorContext> = Symbol("character-editor");

/** 仅弹窗子树读取草稿；游戏舞台继续读取正式资源。 */
export function useCharacterEditorApi() {
  const context = inject(characterEditorKey, null);
  function id() {
    if (context && !context.editId.value) throw new Error("角色编辑草稿尚未就绪");
    return context?.editId.value;
  }
  async function mutate<T>(operation: (editId?: string) => Promise<T>): Promise<T> {
    const editId = id();
    if (context) {
      context.busy.value++;
    }
    try {
      const result = await operation(editId);
      return result;
    } finally {
      if (context) {
        context.busy.value--;
        context.revision.value++;
      }
    }
  }
  return {
    listCharacterAvatars: (roleId: number, clothes: string) =>
      api.listCharacterAvatars(roleId, clothes, id()),
    listCharacterCostumes: (roleId: number) => api.listCharacterCostumes(roleId, id()),
    writeCharacterAvatar: (roleId: number, clothes: string, emotion: string, bytes: number[]) =>
      mutate((editId) => api.writeCharacterAvatar(roleId, clothes, emotion, bytes, editId)),
    deleteCharacterAvatar: (roleId: number, clothes: string, emotion: string) =>
      mutate((editId) => api.deleteCharacterAvatar(roleId, clothes, emotion, editId)),
    manageCharacterCostume: (
      roleId: number,
      action: "create" | "rename" | "remove",
      oldName: string,
      newName: string,
      settings: Record<string, unknown>,
    ) =>
      mutate((editId) =>
        api.manageCharacterCostume(roleId, action, oldName, newName, settings, editId),
      ),
    importLive2d: (roleId: number, sourcePath: string, sourceKind: "directory" | "zip") =>
      mutate((editId) => api.importLive2d(roleId, sourcePath, sourceKind, editId)),
    inspectLive2d: (roleId: number) => api.inspectLive2d(roleId, id()),
    getLive2dFilePath: (roleId: number, filePath: string) =>
      api.getLive2dFilePath(roleId, filePath, id()),
    getLive2dVariantAssets: (roleId: number, variantName: string) =>
      api.getLive2dVariantAssets(roleId, variantName, id()),
  };
}
