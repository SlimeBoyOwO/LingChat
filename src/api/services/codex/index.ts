import { invoke } from "@tauri-apps/api/core";

/** Codex（ChatGPT 订阅）登录状态 */
export interface CodexAuthStatus {
  logged_in: boolean;
  account_id: string | null;
  /** access_token 过期时刻（Unix 毫秒） */
  expires: number | null;
}

/** 设备码登录第一步：用户码与验证地址 */
export interface DeviceLoginStart {
  device_auth_id: string;
  user_code: string;
  interval: number;
  verification_url: string;
}

/** 设备码轮询结果：pending 等待授权 / slow_down 放慢轮询 / complete 完成 */
export interface CodexPollStatus {
  status: "pending" | "slow_down" | "complete";
  account_id: string | null;
}

export interface QuotaWindow {
  /** 剩余百分比（0-100） */
  remaining_percent: number;
  /** 窗口长度（秒）：18000=5 小时窗，604800=7 天周窗 */
  window_seconds: number;
  /** 重置时刻（Unix 秒） */
  reset_at: number | null;
}

export interface RateLimitQuota {
  primary: QuotaWindow | null;
  secondary: QuotaWindow | null;
}

export interface AdditionalQuota {
  name: string;
  quota: RateLimitQuota;
}

export interface CodexUsage {
  rate_limit: RateLimitQuota;
  additional: AdditionalQuota[];
}

export async function codexAuthStatus(): Promise<CodexAuthStatus> {
  return invoke("codex_auth_status");
}

export async function codexStartLogin(): Promise<DeviceLoginStart> {
  return invoke("codex_start_login");
}

export async function codexPollLogin(
  deviceAuthId: string,
  userCode: string
): Promise<CodexPollStatus> {
  return invoke("codex_poll_login", { deviceAuthId, userCode });
}

export async function codexLogout(): Promise<void> {
  return invoke("codex_logout");
}

export async function codexGetQuota(): Promise<CodexUsage> {
  return invoke("codex_get_quota");
}
