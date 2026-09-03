<template>
  <MenuItem :title="$t('settings.playerProfile.title')" size="small">
    <template #header>
      <UserRound :size="20" />
    </template>
    <div class="w-full p-3">
      <!-- 玩家身份卡：名字/副标题/简介 + 编辑按钮（头像功能已整体移除，不再保留圆形头像位） -->
      <div
        class="group relative rounded-2xl border border-white/20 bg-white/10 p-4 backdrop-blur-xl
          transition-all duration-300 hover:-translate-y-1 hover:border-amber-300/50
          hover:shadow-2xl hover:shadow-amber-500/10"
      >
        <div
          class="text-brand absolute -top-2 -left-2 flex h-6 w-6 -rotate-18 transform items-center
            justify-center rounded-full shadow-md"
        >
          <Smile :size="20" />
        </div>

        <div class="flex items-start justify-between gap-4">
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
              <h4 class="text-lg font-bold tracking-wide text-white drop-shadow-md">
                {{ playerName }}
              </h4>
              <span
                v-if="playerSubtitle"
                class="text-brand text-xs font-medium tracking-widest uppercase opacity-80"
              >
                {{ playerSubtitle }}
              </span>
            </div>
            <span class="mt-1.5 block h-1 w-6 rounded-full bg-amber-300"></span>
            <p class="mt-3 line-clamp-3 text-base leading-relaxed text-gray-200/90 opacity-80">
              {{ playerInfo || $t("settings.playerProfile.noInfo") }}
            </p>
          </div>
          <button
            class="shrink-0 rounded-full border border-amber-300/40 bg-amber-400/80 px-5 py-1.5
              text-xs font-bold text-slate-900 shadow-lg shadow-amber-500/20 transition-all
              hover:bg-amber-300"
            @click="openModal"
          >
            <span class="inline-flex items-center gap-1.5">
              <Pencil :size="14" />
              {{ $t("settings.playerProfile.edit") }}
            </span>
          </button>
        </div>
      </div>
    </div>
  </MenuItem>

  <!-- 玩家身份编辑弹窗 -->
  <PlayerProfileModal v-model:visible="modalVisible" :profile="localProfile" @saved="handleSaved" />
</template>

<script setup lang="ts">
  import { computed, onMounted, ref, watch } from "vue";
  import { Pencil, Smile, UserRound } from "lucide-vue-next";
  import { MenuItem } from "../../ui";
  import { useUserStore } from "../../../stores/modules/user/user";
  import type { PlayerProfile } from "../../../api/services/game-info";
  import PlayerProfileModal from "./PlayerProfileModal.vue";

  const userStore = useUserStore();

  const modalVisible = ref(false);
  /** 本地玩家档案副本（用于编辑弹窗） */
  const localProfile = ref<PlayerProfile>({
    user_name: "玩家",
    user_subtitle: "",
    user_prompt: "",
    info: "",
    system_prompt_example: "",
  });

  const playerName = computed(() => localProfile.value.user_name || "玩家");
  const playerSubtitle = computed(() => localProfile.value.user_subtitle || "");
  const playerInfo = computed(() => localProfile.value.info || "");

  async function loadProfile() {
    await userStore.loadPlayerProfile();
    localProfile.value = { ...userStore.playerProfile };
  }

  function openModal() {
    localProfile.value = { ...userStore.playerProfile };
    modalVisible.value = true;
  }

  function handleSaved() {
    localProfile.value = { ...userStore.playerProfile };
  }

  onMounted(loadProfile);

  // 当 user store 更新时同步展示（如初始化数据已加载 / 切人设事件到达）
  watch(
    () => userStore.playerProfile,
    (profile) => {
      localProfile.value = { ...profile };
    },
    { deep: true }
  );
</script>
