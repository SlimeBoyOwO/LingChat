<template>
  <div class="mini-games-page overflow-auto" data-scrollable>
    <div class="mini-games-content">
      <header class="mini-games-header">
        <button class="back" type="button" @click="router.push('/')">
          ← {{ t("views.miniGames.back") }}
        </button>
        <span>LINGCHAT</span>
      </header>
      <h1>{{ t("views.miniGames.title") }}</h1>
      <p class="intro">{{ t("views.miniGames.intro") }}</p>
      <button class="game-card" type="button" @click="router.push('/mini-games/twilight')">
        <div class="game-card-art">
          <img class="background" :src="background" alt="" />
          <img class="character" :src="character" alt="" />
          <span class="game-badge">4 KEYS</span>
        </div>
        <div class="game-card-body">
          <div class="game-card-title">
            <h2>{{ t("views.miniGames.twilightTitle") }}</h2>
            <span aria-hidden="true">↗</span>
          </div>
          <p>{{ t("views.miniGames.twilightDescription") }}</p>
          <div class="game-card-footer">
            <small>{{ t("views.miniGames.twilightMeta") }}</small
            ><span>{{ t("views.miniGames.play") }} →</span>
          </div>
        </div>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { useI18n } from "vue-i18n";
  import { useRouter } from "vue-router";
  import background from "@/assets/minigames/twilight/shrine-dusk.png";
  import character from "@/assets/minigames/twilight/qinling-0.png";

  const { t } = useI18n();
  const router = useRouter();
</script>

<style scoped>
  .mini-games-page {
    width: 100%;
    height: 100%;
    background: radial-gradient(ellipse at 70% 0, #57344375, transparent 65%), #1b1721;
    color: #f8e9cf;
  }
  .mini-games-content {
    width: min(1100px, 100%);
    margin: 0 auto;
    padding: 30px 36px 60px;
  }
  .mini-games-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 50px;
  }
  .mini-games-header > span {
    font: 11px monospace;
    letter-spacing: 4px;
    color: #b99aa9;
  }
  .back {
    border: 1px solid #79566680;
    background: #3c293a55;
    color: #e3c8d0;
    padding: 10px 15px;
    cursor: pointer;
    font-size: 13px;
  }
  h1 {
    font-size: clamp(30px, 4vw, 48px);
    font-weight: 700;
    margin: 0 0 12px;
    letter-spacing: 3px;
  }
  .intro {
    color: #bea2b0;
    font-size: 14px;
    margin-bottom: 30px;
  }
  .game-card {
    display: block;
    width: min(660px, 100%);
    overflow: hidden;
    background: #302332;
    border: 1px solid #89616b75;
    padding: 0;
    color: inherit;
    text-align: left;
    cursor: pointer;
    box-shadow: 0 18px 55px #10091660;
    transition:
      transform 0.15s,
      border-color 0.15s;
  }
  .game-card:hover {
    transform: translateY(-3px);
    border-color: #e9b8a5;
  }
  .game-card:focus-visible,
  .back:focus-visible {
    outline: 2px solid #ffe3ac;
    outline-offset: 4px;
  }
  .game-card-art {
    position: relative;
    aspect-ratio: 16/8;
    overflow: hidden;
  }
  .background {
    width: 100%;
    height: 100%;
    object-fit: cover;
    image-rendering: pixelated;
  }
  .character {
    position: absolute;
    width: 20%;
    left: 42%;
    bottom: 9%;
    image-rendering: pixelated;
  }
  .game-badge {
    position: absolute;
    top: 15px;
    left: 15px;
    background: #2a192cc9;
    border: 1px solid #f8d8b84a;
    color: #f8d9b5;
    font: 10px monospace;
    letter-spacing: 2px;
    padding: 7px 9px;
  }
  .game-card-body {
    padding: 23px 25px;
  }
  .game-card-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 24px;
    letter-spacing: 2px;
    font-weight: 600;
  }
  .game-card-title > span {
    color: #e3ac9a;
    font-size: 27px;
  }
  .game-card-body p {
    color: #c7aab9;
    font-size: 13px;
    margin: 10px 0 23px;
  }
  .game-card-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  .game-card-footer small {
    font-size: 10px;
    color: #a98c9f;
  }
  .game-card-footer > span {
    font-size: 12px;
    color: #f4c99f;
  }
  @media (max-width: 600px) {
    .mini-games-content {
      padding: 20px 18px;
    }
    .mini-games-header {
      margin-bottom: 32px;
    }
    .game-card-body {
      padding: 16px;
    }
    .game-card-footer {
      align-items: flex-start;
      flex-direction: column;
    }
  }
</style>
