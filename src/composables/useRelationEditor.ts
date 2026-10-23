/** 关系行编辑的共享逻辑：行状态、下拉选项（去重置灰 + 失效目标保留）、折成映射。 */
import { ref, type Ref } from "vue";

export interface RelationRow {
  target: string;
  text: string;
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

  /** 用一份 {关系键: 文本} 覆盖行状态（过滤空键空值）。 */
  const syncRows = (relations: Record<string, string> | undefined) => {
    rows.value = Object.entries(relations || {})
      .filter(([target, text]) => !!target && !!text)
      .map(([target, text]) => ({ target, text }));
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

  return { rows, syncRows, addRow, removeRow, fold, optionsFor };
}
