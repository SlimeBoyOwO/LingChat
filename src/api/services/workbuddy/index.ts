import { invoke } from "@tauri-apps/api/core";

/** WorkBuddy（腾讯 CodeBuddy 订阅）登录状态 */
export interface WorkBuddyAuthStatus {
  logged_in: boolean;
  uid: string | null;
  nickname: string | null;
  /** "cn" | "global" */
  realm: string | null;
}

/** 浏览器授权登录第一步：授权链接与登录会话 state */
export interface WorkBuddyLoginStart {
  state: string;
  auth_url: string;
  /** "cn" | "global" */
  realm: string;
}

/** 授权轮询结果：pending 等待授权 / complete 完成 */
export interface WorkBuddyPollStatus {
  status: "pending" | "complete";
  uid: string | null;
  nickname: string | null;
}

export interface CreditPackage {
  name: string;
  remain: number;
  size: number;
  expired: string;
}

export interface WorkBuddyUsage {
  nickname: string;
  uid: string;
  /** "cn" | "global" */
  realm: string;
  remaining: number;
  total: number;
  packages: CreditPackage[];
}

export async function workbuddyAuthStatus(): Promise<WorkBuddyAuthStatus> {
  return invoke("workbuddy_auth_status");
}

export async function workbuddyStartLogin(realm: string): Promise<WorkBuddyLoginStart> {
  return invoke("workbuddy_start_login", { realm });
}

export async function workbuddyPollLogin(
  state: string,
  realm: string,
): Promise<WorkBuddyPollStatus> {
  return invoke("workbuddy_poll_login", { state, realm });
}

export async function workbuddyLogout(): Promise<void> {
  return invoke("workbuddy_logout");
}

export async function workbuddyGetQuota(): Promise<WorkBuddyUsage> {
  return invoke("workbuddy_get_quota");
}
