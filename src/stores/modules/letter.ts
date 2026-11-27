/**
 * 信纸 store：当前会话的信纸页与面板显示状态。
 *
 * 页内容以后端为准，每次展示前重新拉取；面板开合与缩放只是本次界面状态，只有信纸款式会记住。
 */
import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";

export interface LetterPage {
  id: string;
  content: string;
}

export const LETTER_ZOOM_MIN = 0.7;
export const LETTER_ZOOM_MAX = 1.8;

export const LETTER_STYLES = ["classic", "cute", "minimal"] as const;
export type LetterStyle = (typeof LETTER_STYLES)[number];

export const useLetterStore = defineStore("letter", {
  state: () => ({
    pages: [] as LetterPage[],
    index: 0,
    visible: false,
    maximized: false,
    zoom: 1,
    paperStyle: "classic" as LetterStyle,
  }),
  persist: {
    key: "lingchat-letter",
    exclude: ["pages", "index", "visible", "maximized", "zoom"],
  },
  getters: {
    current: (state): LetterPage | null => state.pages[state.index] ?? null,
  },
  actions: {
    async refresh() {
      try {
        this.pages = await invoke<LetterPage[]>("get_session_letters");
      } catch (e) {
        console.warn("[Letter] 读取信纸失败:", e);
      }
      this.index = Math.min(this.index, Math.max(this.pages.length - 1, 0));
    },
    /** 后端写完或改完某页时调用：拉取最新内容并翻到该页。 */
    async showPage(page: number) {
      await this.refresh();
      if (this.pages.length === 0) return;
      this.index = Math.min(Math.max(page, 0), this.pages.length - 1);
      this.visible = true;
    },
    /** 没有信纸时也打开，面板里显示引导语。 */
    async open() {
      await this.refresh();
      this.visible = true;
    },
    hide() {
      this.visible = false;
    },
    turn(delta: number) {
      const next = this.index + delta;
      if (next >= 0 && next < this.pages.length) this.index = next;
    },
    setZoom(zoom: number) {
      this.zoom = Math.min(LETTER_ZOOM_MAX, Math.max(LETTER_ZOOM_MIN, Math.round(zoom * 10) / 10));
    },
    /** 会话边界（切换角色、读档、清空对话）：收起并清空，再拉取新会话已有的信纸。 */
    resetSession() {
      this.visible = false;
      this.maximized = false;
      this.pages = [];
      this.index = 0;
      void this.refresh();
    },
  },
});
