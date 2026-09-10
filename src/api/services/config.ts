import { invoke } from "@tauri-apps/api/core";

export type StructuredConfig = Record<string, any>;

// 单个配置项的类型
export interface ConfigItem {
  key: string;
  value: string;
  description: string;
  type: "text" | "bool" | "textarea" | "path" | "number";
}

export async function fetchEnvConfig(): Promise<StructuredConfig> {
  return invoke("get_settings_tree");
}

export async function saveEnvConfig(values: Record<string, string>): Promise<string> {
  return invoke("save_settings", { values });
}

/**
 * 设置 HDR 模式开关（仅 Windows）。
 * 持久化到后端 settings.json，启动时由 Rust 侧读取并决定 WebView2 色彩配置，重启后生效。
 */
export async function setHdrMode(enabled: boolean): Promise<void> {
  return invoke("set_hdr_mode", { enabled });
}

// ========== 开机自启动（角色桌宠 + TTS 联动） ==========

export interface AutostartStatus {
  /** 系统级开机自启动是否已启用 */
  system_enabled: boolean;
  /** 自启动后是否直接进入桌宠模式 */
  boot_as_pet: boolean;
  /** 开机自启动默认加载的角色 ID（为空则沿用上次角色） */
  pet_role_id: string;
  /** 用于拉起外部 TTS 服务的启动脚本（.bat）路径 */
  tts_launcher_bat: string;
  /** 本次启动是否由「系统开机自启」触发（带 --autostart 参数）；为 true 时 boot_as_pet 才生效 */
  launched_by_autostart: boolean;
  /** 进入桌宠时是否默认开启自动对话（全局：对话场景默认开启自动播放） */
  auto_play: boolean;
  /** 手动启动（非开机自启）时是否以桌宠模式进入 */
  startup_pet_mode: boolean;
  /** 进入桌宠时是否发出「入场问候」（默认关闭） */
  startup_greeting: boolean;
  /** 启动时是否自动拉起/刷新外部 TTS API 服务（全局：无论桌宠还是正常启动均生效） */
  auto_start_tts: boolean;
}

export interface AutostartBootResult {
  tts_type: string;
  embedded: boolean;
  launched: boolean;
  /** 语音服务是否已就绪（内置引擎恒为 true） */
  ready: boolean;
  error?: string | null;
}

/** 查询开机自启动系统状态与配置。 */
export async function getAutostartStatus(): Promise<AutostartStatus> {
  return invoke<AutostartStatus>("autostart_status");
}

/** 切换「开机自启动」（写入系统注册项 / LaunchAgent / .desktop）。 */
export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  return invoke("autostart_set_enabled", { enabled });
}

/** 启动时按当前角色的 TTS 类型决定是否拉起外部语音服务脚本。 */
export async function autostartBootApply(roleId?: number): Promise<AutostartBootResult> {
  return invoke<AutostartBootResult>("autostart_boot_apply", { roleId });
}

export const getEnvConfigByKey = async (key: string): Promise<ConfigItem> => {
  try {
    const data = await invoke("get_setting_by_key", { key });
    return data as ConfigItem;
  } catch (error) {
    console.error("Error fetching config by key:", error);
    throw error;
  }
};

export const getEnvConfigSettings = async (): Promise<StructuredConfig> => {
  try {
    const data = await invoke("get_settings_tree");
    return data as StructuredConfig;
  } catch (error) {
    console.error("Error fetching config env settings:", error);
    throw error;
  }
};

export const saveEnvConfigSettings = async (
  values: Record<string, string>
): Promise<{ status: string; message: string }> => {
  try {
    const message = await invoke("save_settings", { values });
    return { status: "success", message: message as string };
  } catch (error) {
    console.error("Error modifying config env settings:", error);
    throw error;
  }
};
