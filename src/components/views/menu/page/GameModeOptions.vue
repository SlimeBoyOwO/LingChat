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

  const router = useRouter();
  const gameStore = useGameStore();
  const dialogStore = useDialogStore();
  const { t } = useI18n();

  // galgame New Game：先看有没有"当前进行"（last_save_id）。
  //   有 → 询问是否从上次存档继续：是 = 回到当前进行（load_save）；否 = 前往存档页。
  //   无 → 直接开新世界（start_new_game 新建槽，不覆盖旧进度）。
  const startFreeDialogue = async () => {
    gameStore.exitStoryMode();
    try {
      const lastSaveId = await invoke<number | null>("get_last_save_id");
      if (lastSaveId) {
        const ok = await dialogStore.confirm(
          t("views.menu.continueSaveMessage"),
          t("views.menu.continueSaveTitle")
        );
        if (ok) {
          const gameInfo = await invoke<WebInitData>("load_save", { saveId: lastSaveId });
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
</script>
