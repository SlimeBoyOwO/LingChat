<template>
  <div>
    <ToolToggleRow
      :checked="settings.web_search.enabled"
      :label="$t('ui.toolCalls.enableWebSearch')"
      @change="(value) => (settings.web_search.enabled = value)"
    />

    <ToolField :label="$t('ui.toolCalls.provider')" class="mt-2">
      <ToolSelect v-model="settings.web_search.provider">
        <option value="kimi">Kimi /search</option>
        <option value="bocha">BoCha 博查</option>
        <option value="deepseek">{{ $t("ui.toolCalls.providerDeepSeek") }}</option>
        <option value="tavily">Tavily</option>
        <option value="codex">{{ $t("ui.toolCalls.providerCodex") }}</option>
        <option value="custom">{{ $t("ui.toolCalls.providerCustom") }}</option>
      </ToolSelect>
    </ToolField>

    <ToolField :label="$t('ui.toolCalls.apiKey')" class="mt-4">
      <span
        v-if="settings.web_search.provider === 'codex'"
        class="mt-2 block text-sm text-gray-300"
      >
        {{ $t("ui.toolCalls.codexHint") }}
      </span>
      <ToolInput
        v-else
        v-model="settings.web_search.api_key"
        type="password"
        :placeholder="apiKeyPlaceholder"
      />
    </ToolField>
    <p v-if="settings.web_search.provider === 'kimi'" class="mt-2 px-1 text-sm text-gray-300">
      {{ $t("ui.toolCalls.kimiHint") }}
    </p>
    <p v-if="settings.web_search.provider === 'deepseek'" class="mt-2 px-1 text-sm text-gray-300">
      {{ $t("ui.toolCalls.dsHint") }}
    </p>

    <ToolField
      v-if="settings.web_search.provider === 'custom'"
      :label="$t('ui.toolCalls.baseUrl')"
      class="mt-4"
    >
      <span class="mt-1 block text-sm text-gray-300">{{ $t("ui.toolCalls.customHint") }}</span>
      <ToolInput
        v-model="settings.web_search.base_url"
        placeholder="https://api.kimi.com/coding/v1/search"
      />
    </ToolField>

    <ToolField :label="$t('ui.toolCalls.maxResults')" class="mt-4">
      <ToolInput
        v-model="settings.web_search.max_results"
        type="number"
        min="1"
        max="20"
        step="1"
      />
    </ToolField>

    <ToolToggleRow
      :checked="settings.web_search.hide_search_results"
      :label="$t('ui.toolCalls.hideSearchResults')"
      @change="(value) => (settings.web_search.hide_search_results = value)"
    />

    <ToolToggleRow
      :checked="settings.web_search.proxy_enabled"
      :label="$t('ui.toolCalls.proxyEnable')"
      class="mt-2"
      @change="(value) => (settings.web_search.proxy_enabled = value)"
    />
    <p v-if="android" class="mt-1 px-1 text-sm text-amber-300">
      {{ $t("ui.toolCalls.androidProxyHint") }}
    </p>
    <ToolInput
      v-if="settings.web_search.proxy_enabled"
      v-model="settings.web_search.proxy_addr"
      placeholder="http://127.0.0.1:10808"
    />

    <div class="mt-4 flex flex-wrap items-center gap-3">
      <ToolActionButton :disabled="testing" @click="runTest">
        {{ $t("ui.toolCalls.test") }}
      </ToolActionButton>
      <p class="text-sm" :style="{ color: status.color }">{{ status.message }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { testWebSearch, type ToolSettings } from "@/api/services/tool-settings";
import { useTransientStatus } from "@/composables/settings/useTransientStatus";
import { useToolSettingsStore } from "@/stores/modules/settings/tools";
import { isAndroid } from "@/utils/platform";
import ToolField from "@/components/settings/tools/fields/ToolField.vue";
import ToolInput from "@/components/settings/tools/fields/ToolInput.vue";
import ToolSelect from "@/components/settings/tools/fields/ToolSelect.vue";
import ToolActionButton from "@/components/settings/tools/ToolActionButton.vue";
import ToolToggleRow from "@/components/settings/tools/ToolToggleRow.vue";

const settings = defineModel<ToolSettings>({ required: true });
const { t } = useI18n();
const store = useToolSettingsStore();
const android = isAndroid();
const testing = ref(false);
const { status, show: showStatus } = useTransientStatus(5000);

const apiKeyPlaceholder = computed(() => {
  const provider = settings.value.web_search.provider;
  if (provider === "deepseek") return t("ui.toolCalls.dsApiKeyPlaceholder");
  if (provider === "kimi") return t("ui.toolCalls.kimiApiKeyPlaceholder");
  return t("ui.toolCalls.apiKeyPlaceholder");
});

// DeepSeek 使用官方搜索端点，base_url 对它是无效字段，切过去就清掉。
watch(
  () => settings.value.web_search.provider,
  (provider) => {
    if (provider === "deepseek") settings.value.web_search.base_url = "";
  },
);

async function runTest() {
  if (testing.value) return;
  testing.value = true;
  try {
    // 测试前先落盘，确保后端用的是页面上最新配置
    await store.saveDraft("web_search");
    const result = await testWebSearch("LingChat");
    const parsed = JSON.parse(result) as { result_count?: number };
    showStatus(t("ui.toolCalls.testSuccess", { count: parsed.result_count ?? 0 }));
  } catch (error) {
    showStatus(t("ui.toolCalls.testFailed", { message: String(error) }), "red");
  } finally {
    testing.value = false;
  }
}
</script>
