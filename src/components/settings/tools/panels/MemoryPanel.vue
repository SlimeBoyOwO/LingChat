<template>
  <div>
    <div class="border-brand/30 bg-brand/10 rounded-xl border px-4 py-3">
      <ToolToggleRow
        variant="bare"
        :checked="settings.embedding.cloud_enabled"
        :label="$t('ui.toolCalls.embedding.cloudToggle')"
        @change="(value) => (settings.embedding.cloud_enabled = value)"
      />
      <p class="mt-2 text-xs text-gray-400">
        {{ $t("ui.toolCalls.embedding.cloudToggleHint") }}
      </p>
      <p v-if="settings.embedding.cloud_enabled" class="mt-2 text-xs text-amber-300/90">
        {{ $t("ui.toolCalls.embedding.privacyNotice") }}
      </p>
    </div>

    <div v-if="settings.embedding.cloud_enabled" class="mt-4 grid gap-4 sm:grid-cols-2">
      <ToolField :label="$t('ui.toolCalls.embedding.provider')">
        <ToolSelect v-model="settings.embedding.provider">
          <option value="openai">{{ $t("ui.toolCalls.embedding.providerOpenai") }}</option>
          <option value="gemini">{{ $t("ui.toolCalls.embedding.providerGemini") }}</option>
        </ToolSelect>
      </ToolField>
      <ToolField :label="$t('ui.toolCalls.embedding.model')">
        <ToolInput
          v-model="settings.embedding.model"
          :placeholder="$t('ui.toolCalls.embedding.modelPlaceholder')"
        />
      </ToolField>
      <ToolField :label="$t('ui.toolCalls.embedding.baseUrl')">
        <ToolInput
          v-model="settings.embedding.base_url"
          :placeholder="$t('ui.toolCalls.embedding.baseUrlPlaceholder')"
        />
      </ToolField>
      <ToolField :label="$t('ui.toolCalls.embedding.apiKey')">
        <ToolInput v-model="settings.embedding.api_key" type="password" />
      </ToolField>
      <ToolField :label="$t('ui.toolCalls.embedding.timeout')">
        <ToolInput
          v-model="settings.embedding.timeout_secs"
          type="number"
          min="5"
          max="300"
          step="5"
        />
      </ToolField>
    </div>

    <div class="mt-4 flex flex-wrap items-center gap-2">
      <ToolActionButton :disabled="testing" @click="runTest">
        {{ $t("ui.toolCalls.embedding.test") }}
      </ToolActionButton>
      <ToolActionButton @click="runRebuild">
        {{ $t("ui.toolCalls.embedding.rebuild") }}
      </ToolActionButton>
      <p class="text-sm" :style="{ color: status.color }">{{ status.message }}</p>
    </div>

    <div v-if="warming" class="mt-3 px-1">
      <div class="h-2 w-full overflow-hidden rounded-full bg-white/10">
        <div
          class="bg-brand h-full transition-all duration-300"
          :style="{ width: `${warmingPercent}%` }"
        ></div>
      </div>
      <p class="mt-2 text-xs text-gray-400">
        {{
          $t("ui.toolCalls.embedding.warmingProgress", {
            done: warming.done,
            total: warming.total,
            failed: warming.failed,
          })
        }}
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  rebuildEmbeddings,
  testEmbedding,
  type EmbeddingWarmingProgress,
  type ToolSettings,
} from "@/api/services/tool-settings";
import { useTransientStatus } from "@/composables/settings/useTransientStatus";
import { useToolSettingsStore } from "@/stores/modules/settings/tools";
import ToolField from "@/components/settings/tools/fields/ToolField.vue";
import ToolInput from "@/components/settings/tools/fields/ToolInput.vue";
import ToolSelect from "@/components/settings/tools/fields/ToolSelect.vue";
import ToolActionButton from "@/components/settings/tools/ToolActionButton.vue";
import ToolToggleRow from "@/components/settings/tools/ToolToggleRow.vue";

const settings = defineModel<ToolSettings>({ required: true });
const { t } = useI18n();
const store = useToolSettingsStore();

const testing = ref(false);
const { status, show: showStatus } = useTransientStatus(8000);
const warming = ref<EmbeddingWarmingProgress | null>(null);
const warmingPercent = computed(() => {
  const progress = warming.value;
  if (!progress || progress.total <= 0) return 0;
  return Math.min(100, Math.round((progress.done / progress.total) * 100));
});
let unlistenWarming: UnlistenFn | null = null;

async function runTest() {
  if (testing.value) return;
  testing.value = true;
  try {
    // 自检前先落盘，后端用的就是页面上的配置
    await store.saveDraft("memory");
    const result = await testEmbedding();
    showStatus(
      t("ui.toolCalls.embedding.testSuccess", {
        backend: result.backend,
        dimension: result.dimension,
        elapsed: result.elapsedMs,
      }),
    );
  } catch (error) {
    showStatus(t("ui.toolCalls.embedding.testFailed", { message: String(error) }), "red");
  } finally {
    testing.value = false;
  }
}

async function runRebuild() {
  try {
    await store.saveDraft("memory");
    await rebuildEmbeddings();
    showStatus(t("ui.toolCalls.embedding.rebuildStarted"), "#7dd3fc");
  } catch (error) {
    showStatus(t("ui.toolCalls.embedding.testFailed", { message: String(error) }), "red");
  }
}

onMounted(async () => {
  try {
    unlistenWarming = await listen<EmbeddingWarmingProgress>("embedding:warming", (event) => {
      const payload = event.payload;
      warming.value = payload.finished ? null : payload;
      if (payload.finished) {
        showStatus(
          t("ui.toolCalls.embedding.warmingFinished", { failed: payload.failed }),
          payload.failed > 0 ? "#fbbf24" : "#4ade80",
        );
      }
    });
  } catch (error) {
    console.warn("监听向量化预热事件失败:", error);
  }
});

onUnmounted(() => {
  unlistenWarming?.();
  unlistenWarming = null;
});
</script>
