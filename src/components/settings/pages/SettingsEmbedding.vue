<template>
  <div class="h-full overflow-y-auto p-6">
    <!-- 标题栏（与 SettingsAdvanceOther 一致：brand 色 + 下边框分隔） -->
    <header class="border-brand mb-6 border-b pb-4">
      <h2 class="text-brand text-2xl font-semibold">{{ $t("advance.embedding.title") }}</h2>
      <p class="mt-2 text-sm text-white/70">{{ $t("advance.embedding.desc") }}</p>
    </header>

    <!-- 记忆嵌入配置表单（来自后端配置树「记忆嵌入」分类） -->
    <section v-if="embeddingSettings.length" class="mb-6">
      <form @submit.prevent="saveSettings">
        <div v-for="setting in embeddingSettings" :key="setting.key" class="mb-6">
          <SettingItem
            :setting="localizedSetting(setting)"
            @update:value="(value) => (setting.value = value)"
          />
        </div>

        <!-- 保存操作区域（与 SettingsAdvanceOther 保持一致） -->
        <div
          class="bg-brand inline-flex min-w-30 cursor-pointer flex-col gap-2 rounded-lg border-none
            px-5 py-2.5 text-sm font-medium text-white transition-colors duration-200
            hover:bg-[#0056b3]"
          @click="saveSettings"
        >
          <button
            type="button"
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
      </form>
    </section>

    <!-- 记忆嵌入运行状态 -->
    <section class="mb-6 rounded-xl border border-white/10 bg-black/15 p-4">
      <h3 class="mb-1 text-base font-semibold text-white">
        {{ $t("settings.advanceOther.embeddingStatus.title") }}
      </h3>
      <p class="mb-3 text-sm leading-6 text-white/65">
        {{ $t("settings.advanceOther.embeddingStatus.desc") }}
      </p>

      <div class="space-y-1.5 text-sm">
        <div class="flex items-center gap-2">
          <span class="text-white/70">{{
            $t("settings.advanceOther.embeddingStatus.enabled")
          }}</span>
          <span :class="getEnabledClass(embeddingStatus?.enabled)">
            {{ getEnabledText(embeddingStatus?.enabled) }}
          </span>
          <span v-if="!embeddingStatus" class="text-xs text-white/40">{{
            $t("settings.advanceOther.embeddingStatus.loading")
          }}</span>
        </div>
        <div
          v-if="embeddingStatus"
          class="grid grid-cols-1 gap-x-6 gap-y-1.5 text-sm sm:grid-cols-2"
        >
          <div class="flex items-center gap-2">
            <span class="text-white/70">{{
              $t("settings.advanceOther.embeddingStatus.ready")
            }}</span>
            <span :class="embeddingStatus.ready ? 'text-green-400' : 'text-yellow-400'">
              {{ yesNoLabel(embeddingStatus.ready) }}
            </span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-white/70">{{
              $t("settings.advanceOther.embeddingStatus.configured")
            }}</span>
            <span :class="embeddingStatus.configured ? 'text-green-400' : 'text-red-400'">
              {{ yesNoLabel(embeddingStatus.configured) }}
            </span>
          </div>
          <div v-if="embeddingStatus.dim" class="flex items-center gap-2">
            <span class="text-white/70">{{ $t("settings.advanceOther.embeddingStatus.dim") }}</span>
            <span class="text-white">{{ embeddingStatus.dim }}</span>
          </div>
          <div v-if="embeddingStatus.model" class="flex items-center gap-2">
            <span class="text-white/70">{{
              $t("settings.advanceOther.embeddingStatus.model")
            }}</span>
            <span class="break-all text-white">{{ embeddingStatus.model }}</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-white/70">{{
              $t("settings.advanceOther.embeddingStatus.indexLen")
            }}</span>
            <span class="text-white">{{ embeddingStatus.indexLen }}</span>
          </div>
        </div>
        <p
          v-if="embeddingStatus && !embeddingStatus.ready && embeddingStatus.error"
          class="mt-2 text-sm break-all text-red-400"
        >
          {{ embeddingStatus.error }}
        </p>
      </div>

      <div class="mt-3 flex flex-wrap gap-2">
        <Button type="big" :disabled="isLoadingEmbedding" @click="loadEmbeddingStatus">
          <RefreshCw :size="18" :class="{ 'animate-spin': isLoadingEmbedding }" />
          {{ $t("settings.advanceOther.embeddingStatus.refresh") }}
        </Button>
        <Button
          type="big"
          :disabled="isOrganizing || !embeddingStatus?.ready"
          @click="organizeConversation"
        >
          <Sparkles :size="18" :class="{ 'animate-pulse': isOrganizing }" />
          {{ $t("settings.advanceOther.embeddingStatus.organizeConversation") }}
        </Button>
      </div>
      <p
        v-if="organizeResult"
        class="mt-2 text-sm whitespace-normal"
        :class="organizeResult.colorClass"
      >
        {{ organizeResult.message }}
      </p>
    </section>
  </div>
</template>

<script setup lang="ts">
  import { onMounted, reactive, ref } from "vue";
  import { useI18n } from "vue-i18n";
  import SettingItem from "@/components/base/items/SettingItem.vue";
  import { Button } from "@/components/base";
  import {
    getEmbeddingStatus,
    getEnvConfigSettings,
    saveEnvConfigSettings,
    organizeCurrentConversation,
    type EmbeddingStatus,
  } from "@/api/services/config";
  import { RefreshCw, Sparkles } from "lucide-vue-next";

  // 后端配置树（config/tree.rs）中「记忆嵌入」分类名
  const EMBEDDING_CATEGORY = "记忆嵌入";

  const { t, te } = useI18n();

  // 后端配置树的描述均为中文，按键查 i18n 词条做界面本地化；查不到时回退后端原文
  const localizedSetting = (setting: any) => ({
    ...setting,
    description: te(`settings.advanceOther.fields.${setting.key}`)
      ? t(`settings.advanceOther.fields.${setting.key}`)
      : setting.description,
  });

  // --- 配置表单 ---
  const embeddingSettings = ref<any[]>([]);
  const saveStatus = reactive({
    message: "",
    colorClass: "text-green-500",
  });

  // --- 运行状态 ---
  const isLoadingEmbedding = ref(false);
  const embeddingStatus = ref<EmbeddingStatus | null>(null);
  const isOrganizing = ref(false);
  const organizeResult = ref<{ message: string; colorClass: string } | null>(null);

  const getEnabledClass = (enabled?: boolean) => (enabled ? "text-green-400" : "text-gray-400");
  const getEnabledText = (enabled?: boolean) =>
    enabled
      ? t("settings.advanceOther.embeddingStatus.labelOn")
      : t("settings.advanceOther.embeddingStatus.labelOff");
  const yesNoLabel = (value: boolean) =>
    value
      ? t("settings.advanceOther.embeddingStatus.labelYes")
      : t("settings.advanceOther.embeddingStatus.labelNo");

  // Tauri invoke 拒绝时错误可能是字符串；统一提取可读文本
  const errorText = (error: any): string => {
    if (error == null) return "";
    if (typeof error === "string") return error;
    if (typeof error === "object" && typeof error.message === "string") return error.message;
    return String(error);
  };

  const loadEmbeddingStatus = async () => {
    isLoadingEmbedding.value = true;
    try {
      embeddingStatus.value = await getEmbeddingStatus();
    } catch (error) {
      console.error("加载记忆嵌入状态失败:", error);
      embeddingStatus.value = null;
    } finally {
      isLoadingEmbedding.value = false;
    }
  };

  const organizeConversation = async () => {
    isOrganizing.value = true;
    organizeResult.value = null;
    try {
      const result = await organizeCurrentConversation();
      organizeResult.value = {
        message: t("settings.advanceOther.embeddingStatus.organizeResult", {
          added: result.added,
          duplicates: result.duplicates,
          stored: result.stored,
        }),
        colorClass: "text-green-400",
      };
      await loadEmbeddingStatus();
    } catch (error: any) {
      organizeResult.value = {
        message: t("settings.advanceOther.embeddingStatus.organizeError", {
          error: errorText(error),
        }),
        colorClass: "text-red-400",
      };
    } finally {
      isOrganizing.value = false;
    }
  };

  const loadConfig = async () => {
    try {
      const configData = await getEnvConfigSettings();
      const category = configData[EMBEDDING_CATEGORY];
      const subcategories = category?.subcategories || {};
      const firstSubcategory = Object.keys(subcategories)[0];
      if (firstSubcategory) {
        embeddingSettings.value = subcategories[firstSubcategory].settings || [];
      }
    } catch (error: any) {
      console.error(error);
      saveStatus.message = t("settings.advanceOther.msg.loadConfigFailed", {
        error: errorText(error),
      });
      saveStatus.colorClass = "text-red-500";
    }
  };

  const saveSettings = async () => {
    if (!embeddingSettings.value.length) return;

    const formData: Record<string, string> = {};
    embeddingSettings.value.forEach((setting: { key: string; value: string }) => {
      formData[setting.key] = setting.value;
    });

    saveStatus.message = "";
    try {
      saveStatus.message = (await saveEnvConfigSettings(formData)).message;
      saveStatus.colorClass = "text-green-500";
    } catch (error: any) {
      saveStatus.message = t("settings.advanceOther.msg.error", { error: errorText(error) });
      saveStatus.colorClass = "text-red-500";
    } finally {
      setTimeout(() => {
        saveStatus.message = "";
      }, 5000);
    }
  };

  onMounted(async () => {
    await loadConfig();
    await loadEmbeddingStatus();
  });
</script>
