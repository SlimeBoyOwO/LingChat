import { Channel, invoke } from "@tauri-apps/api/core";

export interface CosyvoiceConfig {
  api_key_configured: boolean;
  models: string[];
}

export interface CosyVoiceView {
  voice_id: string;
  name: string;
  model: string;
  status: string | null;
}

export interface CosyVoiceRecord {
  voice_id: string;
  name: string;
  model: string;
  created_at: string | null;
}

export interface CosyvoiceProgress {
  phase: string;
}

export function getConfig(): Promise<CosyvoiceConfig> {
  return invoke<CosyvoiceConfig>("cosyvoice_get_config");
}

export function saveApiKey(apiKey: string): Promise<void> {
  return invoke<void>("cosyvoice_save_api_key", { apiKey });
}

export async function createVoice(
  name: string,
  model: string,
  filePath: string,
  language: string,
  onProgress: (phase: string) => void
): Promise<CosyVoiceRecord> {
  const channel = new Channel<CosyvoiceProgress>();
  channel.onmessage = (event) => onProgress(event.phase);
  return invoke<CosyVoiceRecord>("cosyvoice_create_voice", {
    name,
    model,
    filePath,
    language,
    channel,
  });
}

export function listVoices(): Promise<CosyVoiceView[]> {
  return invoke<CosyVoiceView[]>("cosyvoice_list_voices");
}

/** 查询单音色审核状态（小写 ok/undeployed/deploying…），后端会写回本地缓存 */
export function voiceStatus(voiceId: string): Promise<string> {
  return invoke<string>("cosyvoice_voice_status", { voiceId });
}

export function deleteVoice(voiceId: string): Promise<void> {
  return invoke<void>("cosyvoice_delete_voice", { voiceId });
}

export function synthesizePreview(
  model: string,
  voiceId: string,
  text: string
): Promise<Uint8Array> {
  return invoke<Uint8Array>("cosyvoice_synthesize_preview", { model, voiceId, text });
}
