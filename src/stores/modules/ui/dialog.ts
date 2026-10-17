import { defineStore } from "pinia";
import { i18n } from "@/locales";

interface DialogState {
  isOpen: boolean;
  type: "alert" | "confirm";
  title: string;
  message: string;
  /** 自定义按钮文案，空串表示用 i18n 默认值 */
  okText: string;
  cancelText: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  resolvePromise: ((value?: any) => void) | null;
}

export interface DialogButtonLabels {
  okText?: string;
  cancelText?: string;
}

export const useDialogStore = defineStore("dialog", {
  state: (): DialogState => ({
    isOpen: false,
    type: "alert",
    title: "",
    message: "",
    okText: "",
    cancelText: "",
    resolvePromise: null,
  }),

  actions: {
    alert(message: string, title?: string, labels?: DialogButtonLabels): Promise<void> {
      if (this.isOpen && this.resolvePromise) {
        this.resolvePromise(undefined);
      }
      return new Promise<void>((resolve) => {
        this.isOpen = true;
        this.type = "alert";
        this.title = title || i18n.global.t("stores.dialog.alertTitle");
        this.message = message;
        this.okText = labels?.okText ?? "";
        this.cancelText = labels?.cancelText ?? "";
        this.resolvePromise = resolve;
      });
    },

    confirm(message: string, title?: string, labels?: DialogButtonLabels): Promise<boolean> {
      if (this.isOpen && this.resolvePromise) {
        this.resolvePromise(false);
      }
      return new Promise<boolean>((resolve) => {
        this.isOpen = true;
        this.type = "confirm";
        this.title = title || i18n.global.t("stores.dialog.confirmTitle");
        this.message = message;
        this.okText = labels?.okText ?? "";
        this.cancelText = labels?.cancelText ?? "";
        this.resolvePromise = resolve;
      });
    },

    ok() {
      if (this.resolvePromise) {
        this.resolvePromise(this.type === "confirm" ? true : undefined);
      }
      this.closeDialog();
    },

    cancel() {
      if (this.resolvePromise) {
        this.resolvePromise(false);
      }
      this.closeDialog();
    },

    closeDialog() {
      this.isOpen = false;
      this.resolvePromise = null;
    },
  },
});
