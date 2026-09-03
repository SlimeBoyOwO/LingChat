import { defineStore } from "pinia";
import {
  createPlayerCard,
  deletePlayerCard,
  getPlayerPersona,
  getPlayerProfile,
  getPlayerProfiles,
  setActivePlayerCard,
  setPlayerPersona,
  setPlayerProfile,
  type PlayerPersonaSummary,
} from "@/api/services/player";
import type { PlayerProfile } from "@/api/services/game-info";

export const useUserStore = defineStore("user", {
  state: () => ({
    user_id: "1",
    client_id: "",
    /** 全局玩家档案（解耦玩家与 AI 设定，纯 DB 多卡：当前激活人设） */
    playerProfile: {
      user_name: "玩家",
      user_subtitle: "",
      user_prompt: "",
      info: "",
      system_prompt_example: "",
    } as PlayerProfile,
    /** 玩家人设卡列表（多卡并存） */
    playerProfiles: [] as PlayerPersonaSummary[],
    /** 当前激活人设 id */
    activeProfileId: "default",
    /** player_profile 是否已加载 */
    profileLoaded: false,
  }),
  getters: {
    /** 玩家名（快捷访问） */
    playerName: (state) => state.playerProfile.user_name,
    /** 玩家副标题 */
    playerSubtitle: (state) => state.playerProfile.user_subtitle,
    /** 玩家系统提示词（设定块） */
    playerPrompt: (state) => state.playerProfile.user_prompt,
    /** 玩家简介 */
    playerInfo: (state) => state.playerProfile.info,
    /** 玩家说话风格示例 */
    playerPromptExample: (state) => state.playerProfile.system_prompt_example,
  },
  actions: {
    /** 从后端加载玩家档案 */
    async loadPlayerProfile() {
      try {
        const profile = await getPlayerProfile();
        this.playerProfile = {
          user_name: profile.user_name || "玩家",
          user_subtitle: profile.user_subtitle || "",
          user_prompt: profile.user_prompt || "",
          info: profile.info || "",
          system_prompt_example: profile.system_prompt_example || "",
        };
        this.profileLoaded = true;
      } catch (e) {
        console.warn("加载玩家档案失败:", e);
        this.profileLoaded = false;
      }
    },

    /** 保存玩家档案 */
    async savePlayerProfile(profile: Partial<PlayerProfile>) {
      // 先对当前档案做浅快照：后端保存失败时回滚本地乐观更新，
      // 避免设置弹窗仍停留在“已保存”的表象。
      const snapshot = { ...this.playerProfile };
      this.playerProfile = {
        ...this.playerProfile,
        ...profile,
      };
      try {
        const result = await setPlayerProfile(
          this.playerProfile.user_name,
          this.playerProfile.user_subtitle,
          this.playerProfile.user_prompt,
          this.playerProfile.info,
          this.playerProfile.system_prompt_example
        );
        if (!result?.success) {
          throw new Error("后端返回保存失败");
        }
        return true;
      } catch (e) {
        // 回滚失败写入；原错误继续向上抛，调用方可识别并展示具体原因
        this.playerProfile = snapshot;
        console.error("保存玩家档案失败:", e);
        throw e;
      }
    },

    /** 更新玩家名 */
    setPlayerName(name: string) {
      this.playerProfile.user_name = name;
    },

    /** 更新玩家副标题 */
    setPlayerSubtitle(subtitle: string) {
      this.playerProfile.user_subtitle = subtitle;
    },

    /** 加载玩家人设卡列表 + 当前激活人设 */
    async loadPlayerProfiles() {
      try {
        const res = await getPlayerProfiles();
        this.playerProfiles = res.profiles;
        this.activeProfileId = res.active_profile_id;
      } catch (e) {
        console.warn("加载玩家人设卡列表失败:", e);
      }
    },

    /** 切换当前激活人设；成功后重新加载档案与人设列表 */
    async switchProfile(cardId: string) {
      const res = await setActivePlayerCard(cardId);
      if (!res?.success) throw new Error("后端返回切换失败");
      await this.loadPlayerProfile();
      await this.loadPlayerProfiles();
    },

    /** 读取任意一张人设卡的内容（不改变激活位，供编辑非激活卡时展示） */
    async loadPersonaProfile(cardId: string): Promise<PlayerProfile> {
      const profile = await getPlayerPersona(cardId);
      return {
        user_name: profile.user_name || "玩家",
        user_subtitle: profile.user_subtitle || "",
        user_prompt: profile.user_prompt || "",
        info: profile.info || "",
        system_prompt_example: profile.system_prompt_example || "",
      };
    },

    /** 保存到指定人设卡（仅 DB，不影响激活位与运行时；激活卡请走 savePlayerProfile） */
    async savePersonaProfile(cardId: string, profile: Partial<PlayerProfile>): Promise<boolean> {
      const next = {
        user_name: profile.user_name || "玩家",
        user_subtitle: profile.user_subtitle || "",
        user_prompt: profile.user_prompt || "",
        info: profile.info || "",
        system_prompt_example: profile.system_prompt_example || "",
      };
      const res = await setPlayerPersona(
        cardId,
        next.user_name,
        next.user_subtitle,
        next.user_prompt,
        next.info,
        next.system_prompt_example
      );
      if (!res?.success) throw new Error("后端返回保存失败");
      await this.loadPlayerProfiles();
      return true;
    },

    /** 新建一张玩家人设卡（后端创建即激活）；成功后刷新激活档案与人设列表 */
    async createProfile(
      cardId: string,
      fields: {
        user_name: string;
        user_subtitle?: string;
        user_prompt?: string;
        info?: string;
        system_prompt_example?: string;
      }
    ) {
      const res = await createPlayerCard(
        cardId,
        fields.user_name,
        fields.user_subtitle,
        fields.user_prompt,
        fields.info,
        fields.system_prompt_example
      );
      if (!res?.success) throw new Error("后端返回创建失败");
      await this.loadPlayerProfile();
      await this.loadPlayerProfiles();
    },

    /** 删除一张玩家人设卡；成功后重新加载人设列表 */
    async deleteProfile(cardId: string) {
      const res = await deletePlayerCard(cardId);
      if (!res?.success) throw new Error("后端返回删除失败");
      await this.loadPlayerProfiles();
    },
  },
});
