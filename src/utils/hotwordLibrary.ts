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

/** 词库去重键：分类 + trim 后的 text（跨分类允许同词，同分类内唯一）。 */
export const libraryRowKey = (row: LibraryWord): string =>
  `${row.category_index ?? "none"}\u0000${row.text.trim()}`;

/** 删除分类：丢弃该分类的词条、大于该下标的下标各减 1、移出分类数组。
 *  重排后同分类内可能撞键，需再去重一次，否则后端唯一索引会让整表保存失败。 */
export function removeCategory(draft: LibraryDraft, index: number): void {
  draft.words = draft.words.filter((w) => w.category_index !== index);
  for (const w of draft.words) {
    if (w.category_index !== null && w.category_index > index) {
      w.category_index -= 1;
    }
  }
  draft.categories.splice(index, 1);
  dedupeRows(draft.words, libraryRowKey);
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
