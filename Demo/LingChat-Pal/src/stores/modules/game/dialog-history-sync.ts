import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useGameStore } from ".";
import type { GameMessage } from "./state";

const DIALOG_HISTORY_SYNC_EVENT = "game:dialog-history-sync";
const DIALOG_HISTORY_REQUEST_EVENT = "game:dialog-history-request";

type DialogHistorySyncPayload = {
  sourceWindow: string;
  messages: GameMessage[];
};

type DialogHistoryRequestPayload = {
  requester: string;
};

let isSyncInitialized = false;
let isApplyingRemoteUpdate = false;
let unlistenDialogHistorySync: UnlistenFn | null = null;
let unlistenDialogHistoryRequest: UnlistenFn | null = null;
let lastHistorySignature = "[]";

const cloneDialogHistory = (messages: GameMessage[]): GameMessage[] =>
  messages.map((message) => ({ ...message }));

const buildHistorySignature = (messages: GameMessage[]): string =>
  JSON.stringify(messages);

const createSyncPayload = (
  sourceWindow: string,
  messages: GameMessage[],
): DialogHistorySyncPayload => ({
  sourceWindow,
  messages: cloneDialogHistory(messages),
});

export const setupDialogHistorySync = async (): Promise<void> => {
  if (isSyncInitialized) {
    return;
  }

  isSyncInitialized = true;

  try {
    const currentWindow = getCurrentWindow();
    const currentWindowLabel = currentWindow.label;
    const gameStore = useGameStore();

    lastHistorySignature = buildHistorySignature(gameStore.dialogHistory);

    unlistenDialogHistorySync = await listen<DialogHistorySyncPayload>(
      DIALOG_HISTORY_SYNC_EVENT,
      ({ payload }) => {
        if (!payload || payload.sourceWindow === currentWindowLabel) {
          return;
        }

        const incomingMessages = Array.isArray(payload.messages)
          ? payload.messages
          : [];
        const incomingSignature = buildHistorySignature(incomingMessages);

        if (incomingSignature === lastHistorySignature) {
          return;
        }

        isApplyingRemoteUpdate = true;
        try {
          gameStore.setGameMessages(cloneDialogHistory(incomingMessages));
          lastHistorySignature = incomingSignature;
        } finally {
          isApplyingRemoteUpdate = false;
        }
      },
    );

    unlistenDialogHistoryRequest = await listen<DialogHistoryRequestPayload>(
      DIALOG_HISTORY_REQUEST_EVENT,
      ({ payload }) => {
        if (!payload || payload.requester === currentWindowLabel) {
          return;
        }

        void emit(
          DIALOG_HISTORY_SYNC_EVENT,
          createSyncPayload(currentWindowLabel, gameStore.dialogHistory),
        );
      },
    );

    gameStore.$subscribe((_mutation, state) => {
      const nextSignature = buildHistorySignature(state.dialogHistory);
      if (nextSignature === lastHistorySignature) {
        return;
      }

      lastHistorySignature = nextSignature;
      if (isApplyingRemoteUpdate) {
        return;
      }

      void emit(
        DIALOG_HISTORY_SYNC_EVENT,
        createSyncPayload(currentWindowLabel, state.dialogHistory),
      );
    });

    await emit<DialogHistoryRequestPayload>(DIALOG_HISTORY_REQUEST_EVENT, {
      requester: currentWindowLabel,
    });
  } catch (error) {
    isSyncInitialized = false;
    console.error("dialogHistory 跨窗口同步初始化失败:", error);
  }
};

export const stopDialogHistorySync = (): void => {
  if (unlistenDialogHistorySync) {
    unlistenDialogHistorySync();
    unlistenDialogHistorySync = null;
  }

  if (unlistenDialogHistoryRequest) {
    unlistenDialogHistoryRequest();
    unlistenDialogHistoryRequest = null;
  }

  isSyncInitialized = false;
  isApplyingRemoteUpdate = false;
  lastHistorySignature = "[]";
};
