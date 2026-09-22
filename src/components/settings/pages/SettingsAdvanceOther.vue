<template>
  <div class="flex h-full min-h-0 flex-col md:grid md:grid-cols-[min(30%,280px)_1fr]">
    <!-- 导航菜单：宽屏始终可见；窄屏仅在浏览菜单层级时可见 -->
    <nav
      ref="navContainerRef"
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'menu'"
      @click="() => removeMoreMenu()"
      class="border-brand md:moreMenu:left-0 relative flex flex-col justify-start gap-6.25
        overflow-y-auto border-b transition-all duration-300
        ease-[cubic-bezier(0.18,0.89,0.32,1.00)] md:border-r md:border-b-0"
      :class="['md:left-0', 'translate-y-0', 'moreMenu:translate-y-0']"
    >
      <!-- 滑动指示器 -->
      <div
        ref="indicatorRef"
        class="bg-brand absolute left-2 z-0 w-[calc(100%-40px)] rounded-lg transition-all
          duration-300 ease-[cubic-bezier(0.18,0.89,0.32,1.00)]"
      ></div>

      <div
        class="mt-2 flex items-center gap-1 px-5 text-sm"
        style="color: white; -webkit-text-stroke: 1px black; paint-order: stroke fill"
      >
        {{ $t("settings.advanceOther.restartHint") }}
      </div>

      <div
        v-for="(categoryData, categoryName) in configData"
        :key="categoryName"
        class="flex w-full flex-col gap-1"
      >
        <span
          class="text-brand mb-1 block rounded-lg border border-white/10 bg-white/10 px-3.75 py-2.5
            text-base font-bold
            shadow-[0_8px_32px_rgba(0,0,0,0.1),inset_0_1px_1px_rgba(255,255,255,0.1)]
            backdrop-blur-xl backdrop-saturate-150"
          >{{ catLabel(categoryName) }}</span
        >
        <a
          v-for="(, subcategoryName) in categoryData.subcategories"
          :key="subcategoryName"
          href="#"
          class="adv-nav-link relative z-10 block rounded-lg px-5 py-3 text-white no-underline
            transition-colors duration-200 hover:bg-gray-200 hover:text-black active:font-bold
            active:text-white"
          :class="{
            active: isActive(categoryName, subcategoryName.toString()),
          }"
          @click.prevent="selectSubcategory(categoryName, subcategoryName.toString())"
        >
          {{ subLabel(subcategoryName) }}
        </a>
      </div>
    </nav>

    <!-- 设置内容区域：宽屏始终可见；窄屏仅在浏览内容层级时可见 -->
    <main
      v-show="!uiStore.isNarrowScreen || narrowViewLevel === 'content'"
      class="relative flex h-full justify-center overflow-auto px-10 py-10 md:px-10 md:py-0"
      :class="['translate-y-0', 'moreMenu:translate-y-0']"
    >
      <!-- 窄屏返回按钮 -->
      <button
        v-if="uiStore.isNarrowScreen"
        class="absolute top-0 left-4 flex items-center gap-1.5 rounded-lg px-2 py-1 text-sm
          text-white/70 transition-colors hover:bg-white/10 hover:text-white"
        @click="narrowViewLevel = 'menu'"
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M15 19l-7-7 7-7"
          />
        </svg>
        {{ $t("settings.advanceOther.backToList") }}
      </button>
      <div v-if="selectedSubcategory" class="active w-full">
        <div class="overflow-auto pt-2.5">
          <header class="border-brand mb-6 border-b pb-4">
            <h2 class="text-brand m-0 text-2xl font-semibold">
              {{ subLabel(activeSelection.subcategory ?? "") }}
            </h2>
            <p class="mt-2 text-base">
              {{
                subDesc(activeSelection.subcategory ?? "", selectedSubcategory.description) ||
                $t("settings.advanceOther.subcategoryDesc", {
                  name: subLabel(activeSelection.subcategory ?? ""),
                })
              }}
            </p>
          </header>

          <form @submit.prevent="saveSettings">
            <div v-for="setting in selectedSubcategory.settings" :key="setting.key" class="mb-6">
              <SettingItem
                :setting="localizedSetting(setting)"
                @update:value="(value) => (setting.value = value)"
              />
            </div>
          </form>

          <section
            v-if="activeSelection.category === 'TTS 配置'"
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
            <h3 class="mb-1 text-base font-semibold text-white">
              {{ $t("settings.advanceOther.ttsControl.title") }}
            </h3>
            <p class="mb-3 text-sm leading-6 text-white/65">
              {{ $t("settings.advanceOther.ttsControl.desc") }}
            </p>
            <Button type="big" :disabled="isReconnectingTts" @click="forceReconnectTts">
              <RefreshCw :size="18" :class="{ 'animate-spin': isReconnectingTts }" />
              {{
                isReconnectingTts
                  ? $t("settings.advanceOther.ttsControl.reconnecting")
                  : $t("settings.advanceOther.ttsControl.forceReconnect")
              }}
            </Button>
            <p
              v-if="reconnectStatus.message"
              class="mt-2 text-sm"
              :class="reconnectStatus.colorClass"
            >
              {{ reconnectStatus.message }}
            </p>
          </section>

          <!-- 开机自启动控制（仅「启动项 → 开机自启动」子分类显示） -->
          <section
            v-if="
              isWindows() &&
              activeSelection.category === '启动项' &&
              activeSelection.subcategory === '开机自启动'
            "
            class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4"
          >
            <h3 class="mb-1 text-base font-semibold text-white">
              {{ $t("settings.advanceOther.autostart.title") }}
            </h3>
            <p class="mb-3 text-sm leading-6 text-white/65">
              {{ $t("settings.advanceOther.autostart.desc") }}
            </p>

            <div class="mb-4 flex items-center gap-3">
              <Button type="big" :disabled="togglingAutostart" @click="toggleAutostart">
                {{
                  systemEnabled
                    ? $t("settings.advanceOther.autostart.disable")
                    : $t("settings.advanceOther.autostart.enable")
                }}
              </Button>
              <span class="text-sm" :class="systemEnabled ? 'text-green-400' : 'text-white/50'">
                {{
                  $t("settings.advanceOther.autostart.status", {
                    status: systemEnabled
                      ? $t("settings.advanceOther.autostart.on")
                      : $t("settings.advanceOther.autostart.off"),
                  })
                }}
              </span>
            </div>

            <div class="flex flex-col gap-2">
              <label class="text-sm text-white/80">{{
                $t("settings.advanceOther.autostart.defaultRole")
              }}</label>
              <select
                v-model="selectedRoleId"
                @change="applySelectedRole"
                class="focus:border-brand focus:ring-brand/20 rounded-lg border border-white/10
                  bg-white/10 px-3 py-2.5 text-sm text-white transition-all duration-200
                  focus:ring-2 focus:outline-none"
              >
                <option value="" class="bg-black/60">
                  {{ $t("settings.advanceOther.autostart.noRole") }}
                </option>
                <option
                  v-for="c in characters"
                  :key="c.character_id"
                  :value="c.character_id"
                  class="bg-black/60"
                >
                  {{ c.title }}
                </option>
              </select>
            </div>

            <p class="mt-3 text-xs text-white/45">
              {{ $t("settings.advanceOther.autostart.roleHint") }}
            </p>
          </section>

          <!-- 保存操作区域 -->
          <div
            class="bg-brand inline-flex min-w-30 cursor-pointer flex-col gap-2 rounded-lg
              border-none px-5 py-2.5 text-sm font-medium text-white transition-colors duration-200
              hover:bg-[#0056b3]"
            @click="saveSettings"
          >
            <button
              class="m-0 h-full w-full cursor-pointer border-none bg-transparent p-0 text-white"
            >
              {{ $t("settings.advanceOther.saveButton") }}
            </button>
            <p
              :class="saveStatus.colorClass"
              class="max-w-75 text-xs wrap-break-word whitespace-normal"
            >
              {{ saveStatus.message }}
            </p>
          </div>
        </div>
      </div>
      <div v-else-if="!isLoading && !Object.keys(configData).length" class="active w-full">
        <div class="advanced-settings-container">
          <header>
            <h2 class="adv-title">{{ $t("settings.advanceOther.loadFailed") }}</h2>
            <p class="adv-description">{{ $t("settings.advanceOther.loadFailedDesc") }}</p>
          </header>
        </div>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
  import { ref, onMounted, onUnmounted, computed, reactive, watch, nextTick } from "vue";
  import { useI18n } from "vue-i18n";
  import { useUIStore } from "@/stores/modules/ui/ui";
  import SettingItem from "@/components/base/items/SettingItem.vue";
  import { Button } from "@/components/base";
  import {
    getEnvConfigSettings,
    saveEnvConfigSettings,
    getAutostartStatus,
    setAutostartEnabled,
  } from "@/api/services/config";
  import { reactivateTTS } from "@/api/services/game-info";
  import { characterGetAll } from "@/api/services/character";
  import { switchLlm } from "@/api/services/llm-providers";
  import { isWindows } from "@/utils/platform";
  import { RefreshCw } from "lucide-vue-next";

  // --- 响应式状态定义 ---
  const uiStore = useUIStore();
  const { t, te } = useI18n();

  // 后端配置树的分类/子类/设置项描述均为中文（config/tree.rs），
  // 这里按名称/键查 i18n 词条做界面日文化；查不到时回退后端原文。
  const catLabel = (name: string) =>
    te(`settings.advanceOther.categories.${name}`)
      ? t(`settings.advanceOther.categories.${name}`)
      : name;
  const subLabel = (name: string) =>
    te(`settings.advanceOther.subcategories.${name}`)
      ? t(`settings.advanceOther.subcategories.${name}`)
      : name;
  const subDesc = (name: string, fallback: string) =>
    te(`settings.advanceOther.subcategoryDescs.${name}`)
      ? t(`settings.advanceOther.subcategoryDescs.${name}`)
      : fallback;
  const localizedSetting = (setting: any) => ({
    ...setting,
    description: te(`settings.advanceOther.fields.${setting.key}`)
      ? t(`settings.advanceOther.fields.${setting.key}`)
      : setting.description,
  });
  const narrowViewLevel = ref<"menu" | "content">("menu");
  const isLoading = ref(false);
  const configData = ref<Record<string, any>>({});
  const activeSelection = reactive({
    category: null as string | null,
    subcategory: null as string | null,
  });
  const saveStatus = reactive({
    message: "",
    colorClass: "text-green-500",
  });
  const isReconnectingTts = ref(false);
  const reconnectStatus = reactive({
    message: "",
    colorClass: "text-green-400",
  });
  let reconnectStatusTimer: ReturnType<typeof setTimeout> | null = null;

  // --- 开机自启动控制状态 ---
  const systemEnabled = ref(false);
  const togglingAutostart = ref(false);
  const characters = ref<{ character_id: string; title: string }[]>([]);
  const selectedRoleId = ref<string>("");
  let autostartLoaded = false;

  const emit = defineEmits<{
    "remove-more-menu-from-b": [];
  }>();

  // --- Refs for DOM elements ---
  const navContainerRef = ref<HTMLElement | null>(null);
  const indicatorRef = ref<HTMLElement | null>(null);

  // --- 计算属性 ---
  const selectedSubcategory = computed(() => {
    if (activeSelection.category && activeSelection.subcategory) {
      return configData.value[activeSelection.category]?.subcategories[activeSelection.subcategory];
    }
    return null;
  });

  // --- 方法定义 ---

  const isActive = (category: string, subcategory: string) => {
    return activeSelection.category === category && activeSelection.subcategory === subcategory;
  };

  const selectSubcategory = (category: string, subcategory: string) => {
    activeSelection.category = category;
    activeSelection.subcategory = subcategory;
    // 窄屏下自动切换到内容视图
    if (uiStore.isNarrowScreen) {
      narrowViewLevel.value = "content";
    }
  };

  const saveSettings = async () => {
    if (!selectedSubcategory.value) return;

    const formData: Record<string, string> = {};
    selectedSubcategory.value.settings.forEach((setting: { key: string; value: string }) => {
      formData[setting.key] = setting.value;
    });

    isLoading.value = true;
    saveStatus.message = "";

    try {
      saveStatus.message = (await saveEnvConfigSettings(formData)).message;
      if (Object.prototype.hasOwnProperty.call(formData, "llm.timeout_secs")) {
        await switchLlm();
      }
      saveStatus.colorClass = "text-green-500";

      await loadConfig(false);
    } catch (error: any) {
      saveStatus.message = t("settings.advanceOther.msg.error", { error: error.message });
      saveStatus.colorClass = "text-red-500";
    } finally {
      isLoading.value = false;
      setTimeout(() => {
        saveStatus.message = "";
      }, 5000);
    }
  };

  const forceReconnectTts = async () => {
    if (isReconnectingTts.value) return;

    isReconnectingTts.value = true;
    reconnectStatus.message = t("settings.advanceOther.msg.ttsReactivating");
    reconnectStatus.colorClass = "text-white/70";
    if (reconnectStatusTimer) {
      clearTimeout(reconnectStatusTimer);
      reconnectStatusTimer = null;
    }

    try {
      await reactivateTTS();
      reconnectStatus.message = t("settings.advanceOther.msg.ttsReactivated");
      reconnectStatus.colorClass = "text-green-400";
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      reconnectStatus.message = t("settings.advanceOther.msg.ttsReconnectFailed", {
        error: message,
      });
      reconnectStatus.colorClass = "text-red-400";
    } finally {
      isReconnectingTts.value = false;
      reconnectStatusTimer = setTimeout(() => {
        reconnectStatus.message = "";
        reconnectStatusTimer = null;
      }, 8000);
    }
  };

  // --- 开机自启动面板逻辑 ---
  const loadAutostartPanel = async () => {
    if (autostartLoaded) return;
    autostartLoaded = true;
    try {
      const status = await getAutostartStatus();
      systemEnabled.value = status.system_enabled;
      const chars = await characterGetAll(1, 100);
      characters.value = chars.items.map((c) => ({
        character_id: String(c.character_id),
        title: c.title,
      }));
      selectedRoleId.value = status.pet_role_id;
    } catch (error) {
      console.error("[Autostart] 载入开机自启状态失败:", error);
    }
  };

  const applySelectedRole = async () => {
    try {
      await saveEnvConfigSettings({ "autostart.pet_role_id": selectedRoleId.value });
    } catch (error) {
      console.error("[Autostart] 保存默认启动角色失败:", error);
    }
  };

  const toggleAutostart = async () => {
    if (togglingAutostart.value) return;
    togglingAutostart.value = true;
    const target = !systemEnabled.value;
    try {
      await setAutostartEnabled(target);
      systemEnabled.value = target;
    } catch (error) {
      console.error("[Autostart] 切换开机自启失败:", error);
    } finally {
      try {
        const status = await getAutostartStatus();
        systemEnabled.value = status.system_enabled;
      } catch (error) {
        console.error("[Autostart] 重新读取系统自启状态失败:", error);
      }
      togglingAutostart.value = false;
    }
  };

  const loadConfig = async (selectFirst = true) => {
    isLoading.value = true;
    try {
      configData.value = await getEnvConfigSettings();
      if (!isWindows()) {
        delete configData.value["启动项"]?.subcategories?.["开机自启动"];
      }

      if (selectFirst && Object.keys(configData.value).length > 0) {
        const firstCategory = Object.keys(configData.value)[0];
        if (firstCategory) {
          const firstSubcategory = Object.keys(
            configData.value[firstCategory]?.subcategories || {}
          )[0];

          if (firstCategory && firstSubcategory) {
            selectSubcategory(firstCategory, firstSubcategory);
          }
        }
      }
    } catch (error: any) {
      console.error(error);
      saveStatus.message = t("settings.advanceOther.msg.loadConfigFailed", {
        error: error.message,
      });
      saveStatus.colorClass = "text-red-500";
    } finally {
      isLoading.value = false;
    }
  };

  // --- 导航指示器逻辑 ---
  const updateIndicatorPosition = () => {
    if (!navContainerRef.value || !indicatorRef.value) return;

    const activeLink = navContainerRef.value.querySelector(".adv-nav-link.active") as HTMLElement;

    if (activeLink) {
      const top = activeLink.offsetTop;
      const height = activeLink.offsetHeight;

      if (top) {
        indicatorRef.value.style.top = `${top}px`;
      }
      if (height) {
        indicatorRef.value.style.height = `${height}px`;
      }
    }
  };

  // --- 监听导航容器尺寸变化 ---
  const setupNavResizeObserver = () => {
    if (!navContainerRef.value) return;

    const resizeObserver = new ResizeObserver(() => {
      updateIndicatorPosition();
    });

    resizeObserver.observe(navContainerRef.value);
  };

  // 监视 activeSelection 的变化，并在 DOM 更新后移动指示器
  watch(
    activeSelection,
    async () => {
      await nextTick();
      updateIndicatorPosition();
      if (activeSelection.category === "启动项" && activeSelection.subcategory === "开机自启动") {
        void loadAutostartPanel();
      }
    },
    { deep: true }
  );

  // --- 生命周期钩子 ---
  onMounted(async () => {
    await loadConfig();
    await nextTick();
    updateIndicatorPosition();
    setupNavResizeObserver();
  });

  onUnmounted(() => {
    if (reconnectStatusTimer) {
      clearTimeout(reconnectStatusTimer);
    }
  });

  // --- 窄屏菜单控制 ---
  const addMoreMenu = () => {
    const btnEl = navContainerRef.value as HTMLElement | null;
    if (btnEl) {
      btnEl.classList.add("moreMenu");
    }
  };

  const removeMoreMenu = () => {
    const btnEl = navContainerRef.value as HTMLElement | null;
    if (btnEl) {
      btnEl.classList.remove("moreMenu");
    }
    emit("remove-more-menu-from-b");
  };

  defineExpose({
    addMoreMenu,
  });
</script>
