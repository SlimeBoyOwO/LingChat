import { MAX_CATEGORY_NAME_CHARS, type LibraryWord } from "@/api/services/hotword";
import { dedupeRows } from "@/utils/hotwordRows";

/** 词库草稿：直接就是命令的读写形态（无 id，词条以 category_index 引用分类）。 */
export interface LibraryDraft {
  categories: string[];
  words: LibraryWord[];
}

/** 左栏筛选：`"all"` = 全部；`"none"` = 未分类；数字 = 分类下标。 */
export type CategoryFilter = number | "all" | "none";

/** 分类下拉里「未分类」选项的哨兵值。 */
export const CATEGORY_NONE = "__none__";

export const filterToIndex = (value: string): number | null =>
  value === CATEGORY_NONE ? null : Number(value);

export const indexToFilter = (index: number | null): string =>
  index === null ? CATEGORY_NONE : String(index);

export const countWords = (draft: LibraryDraft, filter: CategoryFilter): number =>
  filter === "all"
    ? draft.words.length
    : draft.words.filter((w) => w.category_index === (filter === "none" ? null : filter)).length;

/**
 * 删除分类（D21 的草稿侧 + spec §5.2.1 的重排不变量）。
 *
 * 词条只存下标不存名字，所以删掉 `index` 后必须：
 *   ① 丢弃挂在该分类下的词条；② 把 `> index` 的下标各减 1；③ 从数组里移掉分类。
 * 漏掉 ② 会让后面的词静默挂到别的分类下——不报错、不崩溃，只有用户看得出来。
 *
 * 重排后可能撞键（两条被删分类之外的词条归并到同一 text），
 * 再跑一次去重兜底：否则后端 `UNIQUE(text)` 会让整个保存失败。
 */
export function removeCategory(draft: LibraryDraft, index: number): void {
  draft.words = draft.words.filter((w) => w.category_index !== index);
  for (const w of draft.words) {
    if (w.category_index !== null && w.category_index > index) {
      w.category_index -= 1;
    }
  }
  draft.categories.splice(index, 1);
  dedupeRows(draft.words);
}

/** 分类名保存前校验。`duplicate` 按 trim 后精确比较，与 DB 的 `UNIQUE(name)` 一致。 */
export function isCategoryNameValid(
  draft: LibraryDraft,
  index: number,
): "ok" | "empty" | "tooLong" | "duplicate" {
  const name = draft.categories[index].trim();
  if (name === "") return "empty";
  if ([...name].length > MAX_CATEGORY_NAME_CHARS) return "tooLong";
  const duplicated = draft.categories.some((other, i) => i !== index && other.trim() === name);
  return duplicated ? "duplicate" : "ok";
}
