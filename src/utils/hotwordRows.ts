/**
 * 热词行的共享编辑语义。`RoleHotword`（角色/全局热词）与 `LibraryWord`（词库）都满足
 * `HotwordRow`，故泛型复用、无需转换——两边的 trim / 去重 / clamp 口径必须完全一致，
 * 否则前端去重键与后端 `UNIQUE` 的 BINARY 比较一旦不一致，保存会整表失败。
 */

/** 权重默认值与口径，与后端 `db/entities/hotword_weight.rs` 对齐。
 *  ⚠️ 权重**不是连续区间**：只允许 `[1, 5]` 或 `50`（`6..=49` 无效）。 */
export const HOTWORD_DEFAULT_WEIGHT = 4;
const HOTWORD_MIN_WEIGHT = 1;
const HOTWORD_WEIGHT_CAP = 5;
const HOTWORD_SUPER_WEIGHT = 50;

/** 权重下拉的档位（顺序即从小到大）。数字框天然表达连续区间，与离散口径不符，故用下拉。 */
export const HOTWORD_WEIGHT_OPTIONS = [1, 2, 3, 4, 5, HOTWORD_SUPER_WEIGHT] as const;

/** 行编辑只关心这两项（另有多余字段如 `role_id` / `category_index` 时仍满足）。 */
export interface HotwordRow {
  text: string;
  weight: number;
}

/** 同 `text` 合并取 max weight、保留首现者（对象与位置都不动）。
 *  用 Map 而非 findIndex 是 O(n) 而非 O(n²)；末尾原地重建数组，调用方持有的引用不失效。 */
export function dedupeRows<T extends HotwordRow>(rows: T[]): void {
  const first = new Map<string, T>();
  const kept: T[] = [];
  for (const row of rows) {
    const key = row.text.trim();
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

/** 词条失焦：trim 后并入同键的首现者（与 DB 的 BINARY 比较口径一致）。 */
export function commitRowText<T extends HotwordRow>(rows: T[], item: T): void {
  item.text = item.text.trim();
  // 空行不归并：后端保存时本就丢弃纯空白条目，合并只会让用户还在编辑的行静默消失。
  if (item.text === "") return;
  const index = rows.indexOf(item);
  if (index === -1) return;
  const first = rows.findIndex((other, i) => i < index && other.text === item.text);
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
