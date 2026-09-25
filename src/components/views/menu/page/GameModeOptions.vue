<template>
  <StartList>
    <StartLine>
      <StartItem class="menu-subitem" @click="startFreeDialogue">{{
        $t("views.menu.freeDialogue")
      }}</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" disabled="true">{{ $t("views.menu.storyMode") }}</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" disabled="true">{{ $t("views.menu.miniGame") }}</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" @click="emit('back')">{{ $t("views.menu.back") }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
import { StartItem, StartLine, StartList } from "../base";
import { useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { useGameStore } from "@/stores/modules/game";
import { applyWebInitData } from "@/stores/modules/game/actions";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import type { WebInitData } from "@/api/services/game-info";
import { invoke } from "@tauri-apps/api/core";

const emit = defineEmits<{
  (e: "back"): void;
  (e: "open-scripts"): void;
  (e: "go-save"): void;
}>();

interface LastSaveInfo {
  save_id: number;
  title: string;
  last_message: string | null;
  script_name: string | null;
}

const truncate = (text: string, max: number) =>
  text.length > max ? `${text.slice(0, max)}…` : text;

const router = useRouter();
const gameStore = useGameStore();
const dialogStore = useDialogStore();
const { t } = useI18n();

// galgame New Game：先看有没有"当前进行"（后端在标记缺失/失效时会回退该角色
// 最新的自动存档槽），并返回摘要（剧本名/最后一句对话）供弹窗标明"上次进行"。
//   有 → 询问是否从上次继续：是 = 回到当前进行（load_save）；否 = 前往存档页。
//   无（无标记且无自动档）→ 直接开新世界（start_new_game 新建槽，不覆盖旧进度）。
const startFreeDialogue = async () => {
  gameStore.exitStoryMode();
  try {
    const lastSave = await invoke<LastSaveInfo | null>("get_last_save_id");
    if (lastSave) {
      const detail = lastSave.script_name
        ? "\n" + t("views.menu.continueDetailScript", { name: lastSave.script_name })
        : lastSave.last_message
          ? "\n" +
            t("views.menu.continueDetailFree", { message: truncate(lastSave.last_message, 40) })
          : "";
      const ok = await dialogStore.confirm(
        t("views.menu.continueSaveMessage") + detail,
        t("views.menu.continueSaveTitle"),
      );
      if (ok) {
        const gameInfo = await invoke<WebInitData>("load_save", { saveId: lastSave.save_id });
        applyWebInitData(gameStore.$state, gameInfo);
        router.push("/chat");
        return;
      }
      emit("go-save");
      return;
    }
    // 无存档 → 直接开新世界
    const data = await invoke<WebInitData>("start_new_game");
    applyWebInitData(gameStore.$state, data);
    router.push("/chat");
  } catch (e) {
    console.error("开始游戏失败:", e);
  }
};

// 前端进入剧情模式（开发中）
const startStoryMode = async () => {
  emit("open-scripts");
};
</script>
