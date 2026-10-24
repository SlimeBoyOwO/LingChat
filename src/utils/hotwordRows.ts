/** 热词行的共享编辑语义：`RoleHotword` 与 `LibraryWord` 都满足 `HotwordRow`，
 *  去重键由调用方传入，须与后端唯一键同形，否则整表保存失败。 */

/** 权重默认值，与后端 hotword_weight.rs 对齐。
 *  权重不是连续区间：只允许 [1, 5] 或 50（6..=49 无效）。 */
export const HOTWORD_DEFAULT_WEIGHT = 4;
const HOTWORD_MIN_WEIGHT = 1;
const HOTWORD_WEIGHT_CAP = 5;
const HOTWORD_SUPER_WEIGHT = 50;

/** 权重下拉档位，从小到大。 */
export const HOTWORD_WEIGHT_OPTIONS = [1, 2, 3, 4, 5, HOTWORD_SUPER_WEIGHT] as const;

/** 行编辑只关心这两项（另有多余字段如 `role_id` / `category_index` 时仍满足）。 */
export interface HotwordRow {
  text: string;
  weight: number;
}

/** 同 key 合并取 max weight、保留首现者；默认 key 为 trim 后的 text，
 *  末尾原地重建数组，调用方持有的引用不失效。 */
export function dedupeRows<T extends HotwordRow>(
  rows: T[],
  keyOf: (row: T) => string = (row) => row.text.trim(),
): void {
  const first = new Map<string, T>();
  const kept: T[] = [];
  for (const row of rows) {
    const key = keyOf(row);
    const seen = first.get(key);
    if (seen) {
      seen.weight = Math.max(seen.weight, row.weight);
    } else {
      first.set(key, row);
      kept.push(row);
    }
  }
  rows.splice(0, rows.length, ...kept);
}

/** 词条失焦：trim 后并入同 key 的首现者（与 DB 的 BINARY 比较口径一致）。 */
export function commitRowText<T extends HotwordRow>(
  rows: T[],
  item: T,
  keyOf: (row: T) => string = (row) => row.text.trim(),
): void {
  item.text = item.text.trim();
  // 空行不归并，否则用户还在编辑的行会静默消失
  if (item.text === "") return;
  const index = rows.indexOf(item);
  if (index === -1) return;
  const key = keyOf(item);
  const first = rows.findIndex((other, i) => i < index && keyOf(other) === key);
  if (first === -1) return;
  rows[first].weight = Math.max(rows[first].weight, item.weight);
  rows.splice(index, 1);
}

/** 失焦时归一到 `[1, 5] ∪ {50}`：清空 / 非数字回默认值；`6..=49` 就近降到 5；`>=50` 归 50。 */
function normalizeWeightInput(raw: unknown): number {
  const text = String(raw).trim();
  const n = Number(text);
  if (text === "" || !Number.isFinite(n)) return HOTWORD_DEFAULT_WEIGHT;
  const w = Math.round(n);
  if (w <= HOTWORD_MIN_WEIGHT) return HOTWORD_MIN_WEIGHT;
  if (w <= HOTWORD_WEIGHT_CAP) return w;
  if (w < HOTWORD_SUPER_WEIGHT) return HOTWORD_WEIGHT_CAP;
  return HOTWORD_SUPER_WEIGHT;
}

export function clampRowWeight<T extends HotwordRow>(item: T): void {
  item.weight = normalizeWeightInput(item.weight);
}
