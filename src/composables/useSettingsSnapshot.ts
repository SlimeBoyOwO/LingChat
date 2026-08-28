import { isWindows } from "@/utils/platform";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { ref } from "vue";

const CAPTURE_TIMEOUT_MS = 800;

const snapshotSrc = ref<string | null>(null);
const snapshotPath = ref<string | null>(null);
const snapshotFailed = ref(false);
const snapshotSessionId = ref(0);

let currentSessionId = 0;
let capturePending: Promise<string | null> | null = null;

function isSupported(): boolean {
  return isWindows();
}

async function capture(): Promise<string | null> {
  if (!isSupported()) {
    // 非 Windows：不使用快照，保持原实时模糊
    return null;
  }

  const myId = ++currentSessionId;
  snapshotSessionId.value = myId;
  snapshotFailed.value = false;

  // 清理上一会话的临时资源（懒清理，避免异常积累）
  const prevPath = snapshotPath.value;
  const prevSrc = snapshotSrc.value;
  // 先清空显示，避免旧图闪烁；但保留 prevPath 以便后台删除
  snapshotSrc.value = null;
  snapshotPath.value = null;

  if (prevPath) {
    // 异步删除，不阻塞
    invoke("cleanup_settings_snapshot", { path: prevPath }).catch(() => {});
  }
  // revoke 兜底（若之前是 blob 则无；convertFileSrc 为 file:// 无需 revoke）
  // 保留以防未来改为 objectURL
  // if (prevSrc && prevSrc.startsWith('blob:')) URL.revokeObjectURL(prevSrc)

  const doCapture = async (): Promise<string | null> => {
    try {
      const path = await invoke<string>("capture_settings_snapshot");
      if (!path) return null;
      return path;
    } catch (e) {
      console.warn("[SettingsSnapshot] capture failed:", e);
      return null;
    }
  };

  const timeout = new Promise<null>((resolve) =>
    setTimeout(() => resolve(null), CAPTURE_TIMEOUT_MS)
  );

  const p = Promise.race([doCapture(), timeout]) as Promise<string | null>;
  capturePending = p;

  let resultPath: string | null = null;
  try {
    resultPath = await p;
  } finally {
    if (capturePending === p) capturePending = null;
  }

  // session 已过期 → 丢弃结果并清理新产生的临时文件
  if (myId !== currentSessionId) {
    if (resultPath) {
      invoke("cleanup_settings_snapshot", { path: resultPath }).catch(() => {});
    }
    return null;
  }

  if (!resultPath) {
    snapshotFailed.value = true;
    snapshotPath.value = null;
    snapshotSrc.value = null;
    return null;
  }

  // 成功：转换为可加载的 file:// URL
  try {
    const src = convertFileSrc(resultPath) + `?v=${myId}`;
    snapshotPath.value = resultPath;
    snapshotSrc.value = src;
    snapshotFailed.value = false;
    return resultPath;
  } catch (e) {
    console.warn("[SettingsSnapshot] convertFileSrc failed:", e);
    snapshotFailed.value = true;
    // 清理已落盘但无法展示的文件
    invoke("cleanup_settings_snapshot", { path: resultPath }).catch(() => {});
    return null;
  }
}

/**
 * 释放指定会话的快照资源。
 * 若 myId 与当前会话不匹配则不清理显示（防止旧清理影响新会话）。
 */
async function release(myId?: number): Promise<void> {
  const targetId = myId ?? snapshotSessionId.value;
  const path = snapshotPath.value;
  const currentId = snapshotSessionId.value;

  // 若指定 myId 且与当前不一致，说明是旧会话的延迟清理：仅删文件，不清显示
  if (myId !== undefined && myId !== currentId) {
    // 无法确定旧 path，使用传入的 path 已在 capture 的丢弃分支处理
    return;
  }

  // 清理显示
  snapshotSrc.value = null;
  snapshotFailed.value = false;
  // 保留 snapshotPath 一份用于删除，清空后异步删
  const toDelete = path ?? snapshotPath.value;
  snapshotPath.value = null;
  snapshotSessionId.value = 0;

  if (toDelete) {
    try {
      await invoke("cleanup_settings_snapshot", { path: toDelete });
    } catch (e) {
      console.warn("[SettingsSnapshot] cleanup failed:", e);
    }
  }
}

function clearForNewSession() {
  // 供 SettingsPanel 在打开新会话时重置失败态（由 capture 内部已处理）
}

export function useSettingsSnapshot() {
  return {
    snapshotSrc,
    snapshotPath,
    snapshotFailed,
    snapshotSessionId,
    isSupported,
    capture,
    release,
    clearForNewSession,
    get currentSessionId() {
      return currentSessionId;
    },
  };
}
