import type { Component } from "vue";

/** 快捷操作菜单项（QuickActionMenuItem）：分体按钮下拉菜单里的一条。 */
export interface QuickActionMenuItem {
  key: string;
  label: string;
  /** 当前生效项，显示勾选标记 */
  active?: boolean;
  disabled?: boolean;
}

/** 快捷操作项（QuickActionItem）：快捷操作栏里一个按钮的描述。 */
export interface QuickActionItem {
  key: string;
  label: string;
  icon?: Component;
  /** primary 主色实心，default 半透明，danger 危险操作 */
  variant?: "primary" | "default" | "danger";
  disabled?: boolean;
  loading?: boolean;
  tooltip?: string;
  /** 存在时按钮右侧出现下拉箭头 */
  menu?: QuickActionMenuItem[];
}
