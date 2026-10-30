/** 关系行编辑的共享逻辑：行状态、下拉选项（去重置灰 + 失效目标保留）、折成映射。 */
import { ref, type Ref } from "vue";

export interface RelationRow {
  target: string;
  text: string;
  /** 角色对玩家身份的初始好感度（只有 `me:` 行使用；null = 跟随角色卡默认） */
  affection?: number | null;
}

/** 下拉目标项；group 为展示用的分组标题（如「AI 角色」/「我的身份」）。 */
export interface RelationTargetOption {
  value: string;
  label: string;
  group?: string;
}

/** 下拉项；与 SelectMenu 的 options 结构对齐。 */
export interface RelationSelectOption {
  label: string;
  value: string;
  group?: string;
  disabled: boolean;
}

export function useRelationEditor(
  targets: Ref<RelationTargetOption[]>,
  /** 目标已不存在时的选项文案；传了才保留原值并标注 */
  missingLabel?: (key: string) => string,
) {
  const rows = ref<RelationRow[]>([]);

  /** 用一份 {关系键: 文本} 覆盖行状态（过滤空键空值）；给了好感度表时一并带上。 */
  const syncRows = (
    relations: Record<string, string> | undefined,
    affections?: Record<string, number | null>,
  ) => {
    const texts = relations ?? {};
    const affects = affections ?? {};
    const keys = new Set([...Object.keys(texts), ...Object.keys(affects)]);
    rows.value = [...keys]
      .filter((key) => !!key && (!!texts[key]?.trim() || affects[key] != null))
      .map((key) => ({
        target: key,
        text: texts[key] ?? "",
        affection: affects[key] ?? null,
      }));
  };

  /** 新增一行；默认目标为空串（不预选）。 */
  const addRow = (target = "") => {
    rows.value.push({ target, text: "" });
  };

  const removeRow = (idx: number) => {
    rows.value.splice(idx, 1);
  };

  /** 折成 {关系键: 文本}；空目标 / 空文本丢弃（后端也会再过滤一次）。 */
  const fold = (): Record<string, string> => {
    const map: Record<string, string> = {};
    for (const row of rows.value) {
      if (row.target && row.text.trim()) map[row.target] = row.text.trim();
    }
    return map;
  };

  /** 折成 {关系键: 初始好感度}；只收我的身份行，null 表示恢复角色卡默认。 */
  const foldAffections = (): Record<string, number | null> => {
    const map: Record<string, number | null> = {};
    for (const row of rows.value) {
      if (row.target.startsWith("me:")) map[row.target] = row.affection ?? null;
    }
    return map;
  };

  /** 输入框写回：空串或非法数字视为未设置。 */
  const setAffection = (row: RelationRow, value: string) => {
    const parsed = Number(value);
    row.affection =
      value.trim() === "" || Number.isNaN(parsed)
        ? null
        : Math.min(100, Math.max(-100, Math.round(parsed)));
  };

  /** 下拉选项：其他行已选过的目标置灰不可再选（本行当前值除外）。 */
  const optionsFor = (row: RelationRow): RelationSelectOption[] => {
    const used = new Set(rows.value.map((r) => r.target).filter(Boolean));
    const options: RelationSelectOption[] = targets.value.map((item) => ({
      label: item.label,
      value: item.value,
      group: item.group,
      disabled: item.value !== row.target && used.has(item.value),
    }));
    if (missingLabel && row.target && !targets.value.some((t) => t.value === row.target)) {
      options.unshift({
        label: missingLabel(row.target),
        value: row.target,
        disabled: false,
      });
    }
    return options;
  };

  return { rows, syncRows, addRow, removeRow, fold, foldAffections, setAffection, optionsFor };
}
