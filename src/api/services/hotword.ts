import { invoke } from "@tauri-apps/api/core";

/**
 * 角色热词与全局热词（后端 `api/hotword.rs`）。
 *
 * 错误不在这里包一层：命令返回的已经是中文可读信息，调用方（角色设置弹窗）
 * 自己决定怎么提示，与 `character.ts` 里 `importLive2d` 那批直连 invoke 一致。
 */

/** 一条角色热词。 */
export interface RoleHotword {
  text: string;
  /** `[1, 5] ∪ {50}`，默认 4。 */
  weight: number;
  /** 归属角色（UI 归因展示用）。 */
  role_id: number;
}

/**
 * 角色热词草稿：热词 tab 的编辑态。由弹窗（父组件）持有，编辑只进内存，
 * 随底部「保存」与其余角色设置一次提交；「取消」即丢弃（D6）。
 */
export interface RoleHotwordDraft {
  hotwords: RoleHotword[];
}

/** 热词的 wire 契约（保存与识别两条链路同一份，`asr.ts` 直接用它）：
 *  与后端 `HotwordInput::Weighted` 同形。`RoleHotword` 多一个 `role_id`（后端忽略）。 */
export interface HotwordInput {
  text: string;
  weight: number;
}

/** 单条词条的长度判据，与后端 `hotword_repo.rs` 的 `validate_text_len` 对齐：
 *  **两套判据，按是否含非 ASCII 分流**——含非 ASCII 时按总字符数，纯 ASCII 时按
 *  空格切分的片段数。⚠️ 两者不是同一个数的变体（`DeepSeek-harness` 16 字符却合规）。 */
export const MAX_HOTWORD_CHARS = 15;
export const MAX_HOTWORD_SEGMENTS = 7;

/** 每角色条数上限，与后端 `hotword_repo.rs` 的 `MAX_ROWS_PER_ROLE` 对齐。 */
export const MAX_HOTWORDS_PER_ROLE = 200;

/** 按**码点**计数，与后端 `chars().count()` 同口径——`String.length` 数的是
 *  UTF-16 码元，会把 emoji 等代理对算成 2，与后端的拒收边界不一致。 */
export const hotwordCharCount = (text: string) => [...text].length;

/** 按空白切分的片段数，与后端 `split_whitespace().count()` 同口径。 */
export const hotwordSegmentCount = (text: string) => text.split(/\s+/).filter(Boolean).length;

/** 是否纯 ASCII（判据分流依据），与后端 `str::is_ascii` 同口径。 */
export const hotwordTextIsAscii = (text: string) => /^[\x00-\x7F]*$/.test(text);

/** 词条触犯了哪条判据；`"ok"` = 合规。口径与后端一致：**先 trim**，尾随空白不算。 */
export function hotwordLimitKind(text: string): "ok" | "chars" | "segments" {
  const t = text.trim();
  if (hotwordTextIsAscii(t)) {
    return hotwordSegmentCount(t) > MAX_HOTWORD_SEGMENTS ? "segments" : "ok";
  }
  return hotwordCharCount(t) > MAX_HOTWORD_CHARS ? "chars" : "ok";
}

/** 保存会被后端拒收的超长词条（口径与后端一致：**先 trim 再判**）。 */
export const isHotwordTextOverlong = (text: string) => hotwordLimitKind(text) !== "ok";

/** 单个角色的热词（角色设置页读取用）。 */
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

/** 保存全局热词（整表覆盖，后端一个事务）。 */
export const saveGlobalHotwords = (hotwords: HotwordInput[]) =>
  invoke<void>("save_global_hotwords", { hotwords });

/** 词库词条。读写共用——无行 id（拷贝式下没有引用者）。 */
export interface LibraryWord {
  /** `null` = 未分类。 */
  category_index: number | null;
  text: string;
  /** `[1, 5] ∪ {50}`。 */
  weight: number;
}

/** 词库的读写形态：分类名数组（顺序 = 插入顺序）+ 词条数组。 */
export interface HotwordLibrary {
  categories: string[];
  words: LibraryWord[];
}

/** 库词条数上限，与后端 `hotword_repo.rs` 的 `MAX_ROWS_PER_LIBRARY` 对齐。 */
export const MAX_LIBRARY_ROWS = 2000;

/** 库分类数上限，与后端 `MAX_CATEGORIES` 对齐。 */
export const MAX_LIBRARY_CATEGORIES = 100;

/** 分类名字符数上限，与后端 `MAX_CATEGORY_NAME_CHARS` 对齐。 */
export const MAX_CATEGORY_NAME_CHARS = 30;

/** 读取整个词库。**无缓存**：每次打开弹窗/选择器都现读。 */
export const getHotwordLibrary = () => invoke<HotwordLibrary>("get_hotword_library");

/** 保存整个词库（整表替换，两表一个事务，失败即整体回滚）。
 *  参数名是 `categories` / `words`：无分隔符词名不做 camelCase 变换，原样传。 */
export const saveHotwordLibrary = (categories: string[], words: LibraryWord[]) =>
  invoke<void>("save_hotword_library", { categories, words });
