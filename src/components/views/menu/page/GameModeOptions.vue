<template>
  <StartList>
    <StartLine>
      <StartItem class="menu-subitem" @click="startFreeDialogue">{{
        $t("views.menu.freeDialogue")
      }}</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" @click="startStoryMode">{{
        $t("views.menu.storyMode")
      }}</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" @click="router.push('/mini-games')">{{
        $t("views.menu.miniGame")
      }}</StartItem>
    </StartLine>

    <StartLine>
      <StartItem class="menu-subitem" @click="emit('back')">{{ $t("views.menu.back") }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
  import { StartItem, StartLine, StartList } from "../base";
  import { useRouter } from "vue-router";
  import { useGameStore } from "@/stores/modules/game";
  import { eventQueue } from "@/core/events/event-queue";

  const emit = defineEmits<{
    (e: "back"): void;
    (e: "open-scripts"): void;
  }>();

  const router = useRouter();
  const gameStore = useGameStore();

  const startFreeDialogue = () => {
    eventQueue.clear();
    gameStore.exitStoryMode();
    router.push("/chat");
  };

  // 进入剧情模式：切到剧本列表页
  const startStoryMode = () => {
    emit("open-scripts");
  };
</script>
