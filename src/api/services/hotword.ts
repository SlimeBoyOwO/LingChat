import { invoke } from "@tauri-apps/api/core";

/** 角色热词与全局热词（后端 api/hotword.rs）。错误直接透传，由调用方决定提示。 */

export interface RoleHotword {
  text: string;
  /** `[1, 5] ∪ {50}`，默认 4。 */
  weight: number;
  /** 归属角色（UI 归因展示用）。 */
  role_id: number;
}

/** 角色热词草稿：弹窗持有，编辑只进内存，随角色设置一次提交。 */
export interface RoleHotwordDraft {
  hotwords: RoleHotword[];
}

/** 热词的 wire 契约，与后端 HotwordInput::Weighted 同形（RoleHotword 多出的 role_id 后端忽略）。 */
export interface HotwordInput {
  text: string;
  weight: number;
}

/** 单条词条的长度判据，与后端 validate_text_len 对齐：含非 ASCII 按字符数，纯 ASCII 按空白片段数。 */
export const MAX_HOTWORD_CHARS = 15;
export const MAX_HOTWORD_SEGMENTS = 7;

/** 每角色条数上限，与后端 `hotword_repo.rs` 的 `MAX_ROWS_PER_ROLE` 对齐。 */
export const MAX_HOTWORDS_PER_ROLE = 200;

/** 按码点计数，与后端 chars().count() 同口径（String.length 按 UTF-16 码元，会算错代理对）。 */
export const hotwordCharCount = (text: string) => [...text].length;

/** 按空白切分的片段数，与后端 `split_whitespace().count()` 同口径。 */
export const hotwordSegmentCount = (text: string) => text.split(/\s+/).filter(Boolean).length;

/** 是否纯 ASCII（判据分流依据），与后端 `str::is_ascii` 同口径。 */
export const hotwordTextIsAscii = (text: string) => /^[\x00-\x7F]*$/.test(text);

/** 词条触犯的判据；ok = 合规。与后端一致先 trim，尾随空白不算。 */
export function hotwordLimitKind(text: string): "ok" | "chars" | "segments" {
  const t = text.trim();
  if (hotwordTextIsAscii(t)) {
    return hotwordSegmentCount(t) > MAX_HOTWORD_SEGMENTS ? "segments" : "ok";
  }
  return hotwordCharCount(t) > MAX_HOTWORD_CHARS ? "chars" : "ok";
}

export const isHotwordTextOverlong = (text: string) => hotwordLimitKind(text) !== "ok";

export const getRoleHotwords = (roleId: number) =>
  invoke<RoleHotword[]>("get_role_hotwords", { roleId });

/** 场景级热词：主角 → present 顺序 → 组内插入顺序，不去重（运行时注入用）。 */
export const getSceneHotwords = (mainRoleId: number | null, presentRoleIds: number[]) =>
  invoke<RoleHotword[]>("get_scene_hotwords", { mainRoleId, presentRoleIds });

/** 一次提交角色的热词（整表覆盖，后端一个事务）。 */
export const saveRoleHotwords = (roleId: number, hotwords: HotwordInput[]) =>
  invoke<void>("save_role_hotwords", { roleId, hotwords });

/** 全局热词（不随角色）。与 `RoleHotword` 同形，`role_id` 恒为 0（占位，不入库、不上行）。 */
export const getGlobalHotwords = () => invoke<RoleHotword[]>("get_global_hotwords");

/** 同上：整表覆盖，一个事务。 */
export const saveGlobalHotwords = (hotwords: HotwordInput[]) =>
  invoke<void>("save_global_hotwords", { hotwords });

/** 词库词条（无行 id，读写共用）。 */
export interface LibraryWord {
  /** `null` = 未分类。 */
  category_index: number | null;
  text: string;
  weight: number;
}

/** 词库的读写形态：分类名数组（顺序 = 插入顺序）+ 词条数组。 */
export interface HotwordLibrary {
  categories: string[];
  words: LibraryWord[];
}

/** 以下上限与后端 `hotword_repo.rs` 对齐。 */
export const MAX_LIBRARY_ROWS = 2000;
export const MAX_LIBRARY_CATEGORIES = 100;
export const MAX_CATEGORY_NAME_CHARS = 30;

/** 读取整个词库（无缓存，每次现读）。 */
export const getHotwordLibrary = () => invoke<HotwordLibrary>("get_hotword_library");

/** 保存整个词库（整表替换，两表一个事务）。 */
export const saveHotwordLibrary = (categories: string[], words: LibraryWord[]) =>
  invoke<void>("save_hotword_library", { categories, words });
