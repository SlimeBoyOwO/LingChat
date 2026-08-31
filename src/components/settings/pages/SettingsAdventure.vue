<template>
  <MenuPage>
    <!-- 独立剧本部分 -->
    <MenuItem :title="$t('settings.adventure.standalone.title')">
      <template #header>
        <FileText :size="20" />
      </template>

      <!-- 独立剧本列表 -->
      <div
        v-if="standaloneScriptsLoading"
        class="flex flex-col items-center justify-center py-8 text-gray-400"
      >
        <div
          class="border-brand mb-2 h-12 w-12 animate-spin rounded-full border-4
            border-t-transparent"
        ></div>
        <p>{{ $t("settings.shared.loading") }}</p>
      </div>

      <div
        v-else-if="standaloneScripts.length === 0"
        class="flex flex-col items-center justify-center py-12 text-gray-400"
      >
        <div class="mb-4 flex h-20 w-20 items-center justify-center rounded-full bg-gray-800/50">
          <FileText :size="40" class="text-gray-500" />
        </div>
        <p class="mb-2 text-lg">{{ $t("settings.adventure.standalone.empty") }}</p>
        <p class="mb-6 text-sm text-gray-500">
          {{ $t("settings.adventure.standalone.emptyDesc") }}
        </p>
      </div>

      <div v-else class="space-y-4">
        <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
          <div
            v-for="script in standaloneScripts"
            :key="script.script_name"
            class="group hover:border-brand/50 relative flex cursor-pointer flex-col rounded-xl
              border border-gray-700 bg-gray-800/50 p-4 transition-all duration-300
              hover:bg-gray-800/80"
          >
            <div class="mb-3 flex items-center justify-between">
              <h3 class="truncate text-lg font-bold text-white">{{ script.script_name }}</h3>
              <span
                class="bg-brand/20 text-brand border-brand/30 rounded-full border px-3 py-1 text-xs
                  font-medium"
              >
                {{ $t("settings.adventure.standalone.badge") }}
              </span>
            </div>

            <p v-if="script.description" class="mb-4 line-clamp-3 flex-1 text-sm text-gray-300">
              {{ script.description }}
            </p>
            <p v-else class="mb-4 text-sm text-gray-500 italic">
              {{ $t("settings.adventure.standalone.noDesc") }}
            </p>

            <div class="mt-auto flex items-center justify-between">
              <span v-if="script.intro_chapter" class="text-xs text-gray-400">
                {{
                  $t("settings.adventure.standalone.chapterSelect", {
                    chapter: script.intro_chapter,
                  })
                }}
              </span>
              <Button type="select" size="sm" @click.stop="startStandaloneScript(script)">
                {{ $t("settings.adventure.standalone.play") }}
              </Button>
            </div>
          </div>
        </div>
      </div>
    </MenuItem>

    <!-- 羁绊冒险部分 -->
    <MenuItem :title="$t('settings.adventure.bond.title')">
      <template #header>
        <Book :size="20" />
      </template>

      <!-- 如果没有选中角色 -->
      <div
        v-if="!currentCharacter"
        class="flex flex-col items-center justify-center py-12 text-gray-400"
      >
        <div class="mb-4 flex h-20 w-20 items-center justify-center rounded-full bg-gray-800/50">
          <Book :size="40" class="text-gray-500" />
        </div>
        <p class="mb-2 text-lg">{{ $t("settings.adventure.bond.noCharacter") }}</p>
        <p class="mb-6 text-sm text-gray-500">
          {{ $t("settings.adventure.bond.noCharacterDesc") }}
        </p>
        <Button type="big" @click="goToCharacterTab">
          {{ $t("settings.adventure.bond.goCharacter") }}
        </Button>
      </div>

      <!-- 如果已选中角色 -->
      <div v-else class="space-y-4">
        <div class="flex items-center gap-4 rounded-xl border border-white/10 bg-gray-900/50 p-4">
          <img
            :src="currentCharacterAvatar"
            class="h-16 w-16 rounded-full border-2 border-indigo-500/50 object-cover"
            :alt="$t('settings.adventure.bond.avatarAlt')"
          />
          <div class="min-w-0 flex-1">
            <h3 class="truncate text-xl font-bold text-white">{{ currentCharacter.roleName }}</h3>
            <p class="truncate text-sm text-gray-400">
              {{ currentCharacter.roleSubTitle || $t("settings.adventure.bond.noSubtitle") }}
            </p>
          </div>
          <div class="shrink-0">
            <Button type="big" @click="goToCharacterTab">
              {{ $t("settings.adventure.bond.switchCharacter") }}
            </Button>
          </div>
        </div>

        <div v-if="gameStore.mainRole">
          <AdventurePanel :character-folder="gameStore.mainRole.character_folder" />
        </div>
      </div>
    </MenuItem>

    <MenuItem :title="$t('settings.adventure.workshop.title')" size="small">
      <template #header>
        <Birdhouse :size="20" />
      </template>
      <Button type="big" @click="openCreativeWeb">{{
        $t("settings.adventure.workshop.enter")
      }}</Button>
    </MenuItem>

    <MenuItem :title="$t('settings.adventure.createScript.title')" size="small">
      <template #header>
        <UserPlus :size="20" />
      </template>
      <div class="space-y-2">
        <Button type="big" @click="openGuideWeb">{{
          $t("settings.adventure.createScript.guide")
        }}</Button>
      </div>
    </MenuItem>
  </MenuPage>
</template>

<script setup lang="ts">
  import { computed, ref, onMounted, watch } from "vue";
  import { useRouter } from "vue-router";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { MenuPage, MenuItem } from "../../ui";
  import { Button } from "@/components/base";
  import AdventurePanel from "./Adeventure/AdventurePanel.vue";
  import { useGameStore } from "@/stores/modules/game";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import { getAvatarFile } from "@/api/services/character";
  import { Birdhouse, Book, FileText, UserPlus } from "lucide-vue-next";
  import {
    getStandaloneScriptList,
    startScript as startScriptApi,
  } from "@/api/services/script-info";
  import type { ScriptSummary } from "@/api/services/script-info";

  const gameStore = useGameStore();
  const uiStore = useUIStore();
  const router = useRouter();
  // 独立剧本相关状态
  const standaloneScripts = ref<ScriptSummary[]>([]);
  const standaloneScriptsLoading = ref(true);

  // 获取当前主角
  const currentCharacter = computed(() => gameStore.mainRole);

  // 获取角色头像
  const currentCharacterAvatar = ref("");

  async function updateCharacterAvatar() {
    if (gameStore.mainRole?.character_folder) {
      try {
        const path = await getAvatarFile(
          gameStore.mainRole.character_folder,
          gameStore.mainRole.clothesName
        );
        currentCharacterAvatar.value = convertFileSrc(path);
      } catch {
        currentCharacterAvatar.value = "";
      }
    } else {
      currentCharacterAvatar.value = "";
    }
  }

  watch(() => gameStore.mainRole?.character_folder, updateCharacterAvatar, { immediate: true });

  // 跳转到角色标签页
  const goToCharacterTab = () => {
    uiStore.setSettingsTab("character");
  };

  // 开始游玩独立剧本
  const startStandaloneScript = async (script: ScriptSummary) => {
    try {
      await startScriptApi(script.script_name);
      // 可选：关闭设置面板，开始剧本
      uiStore.showSettings = false;
    } catch (error) {
      console.error("启动独立剧本失败:", error);
    }
  };

  // 获取独立剧本列表
  const fetchStandaloneScripts = async () => {
    try {
      standaloneScriptsLoading.value = true;
      const scripts = await getStandaloneScriptList();
      standaloneScripts.value = scripts;
    } catch (error) {
      console.error("获取独立剧本列表失败:", error);
      standaloneScripts.value = [];
    } finally {
      standaloneScriptsLoading.value = false;
    }
  };

  const openCreativeWeb = () => {
    // 云端创意工坊已迁移为主菜单「创意工坊」二级菜单的独立路由页
    router.push("/workshop");
  };

  const openGuideWeb = () => {
    openUrl("https://slimeboyowo.github.io/LingBlog/blog/projects/ling-chat/script-guide");
  };

  // 组件挂载时获取独立剧本列表
  onMounted(() => {
    fetchStandaloneScripts();
  });
</script>

<style scoped>
  /* 可以添加自定义样式 */
</style>
