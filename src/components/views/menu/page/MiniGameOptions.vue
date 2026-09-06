<template>
  <StartList>
    <StartLine>
      <StartItem class="menu-subitem" @click="router.push('/mini-games/twilight')">{{
        $t("views.miniGames.twilightTitle")
      }}</StartItem>
    </StartLine>
    <StartLine>
      <StartItem class="menu-subitem" @click="emit('back')">{{ $t("views.menu.back") }}</StartItem>
    </StartLine>
  </StartList>
</template>

<script setup lang="ts">
  import { useRouter } from "vue-router";
  import { StartItem, StartLine, StartList } from "../base";

  const router = useRouter();
  const emit = defineEmits<{ (e: "back"): void }>();
</script>
