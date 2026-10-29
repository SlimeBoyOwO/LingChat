<template>
  <Transition name="modal">
    <div
      v-if="visible && plugin"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4 backdrop-blur-sm"
      @click="requestClose"
    >
      <div
        class="glass-effect flex h-[85dvh] w-full max-w-4xl flex-col overflow-hidden rounded-3xl text-white"
        @click.stop
      >
        <!-- 头部 -->
        <div
          class="flex items-center justify-between border-b border-white/10 bg-white/5 p-4 sm:p-6"
        >
          <div class="flex min-w-0 items-center gap-4">
            <div
              class="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-white/10 shadow-inner"
            >
              <Icon icon="package" />
            </div>
            <div class="min-w-0">
              <h2 class="m-0 truncate text-xl font-bold drop-shadow-[0_2px_4px_rgba(0,0,0,0.3)]">
                {{ plugin.name }}
              </h2>
              <p class="m-0 truncate text-sm text-white/50">
                v{{ plugin.version
                }}<template v-if="plugin.author"> · {{ plugin.author }}</template>
              </p>
            </div>
          </div>
          <button
            class="flex h-9 w-9 shrink-0 cursor-pointer items-center justify-center rounded-full border-none bg-white/10 text-white transition-all duration-200 hover:rotate-90 hover:bg-white/20"
            @click="requestClose"
          >
            <Icon icon="close" :size="24" />
          </button>
        </div>

        <div class="flex min-h-0 flex-1 flex-col overflow-hidden sm:flex-row">
          <!-- 标签栏：窄屏横排，宽屏竖排 -->
          <div
            class="tab-sidebar-scroll flex w-full shrink-0 flex-row gap-2 overflow-x-auto border-b border-white/10 bg-black/10 p-2 sm:w-44 sm:flex-col sm:overflow-y-auto sm:border-r sm:border-b-0 sm:p-3"
          >
            <button
              v-for="tab in tabs"
              :key="tab.id"
              class="shrink-0 cursor-pointer rounded-xl border-none bg-transparent px-4 py-2.5 text-left font-medium text-white/60 transition-all duration-200 hover:bg-white/5 hover:text-white"
              :class="{
                'bg-[rgba(94,114,228,0.2)] font-semibold! text-[#79d9ff]!': activeTab === tab.id,
              }"
              @click="activeTab = tab.id"
            >
              {{ tab.label }}
            </button>
          </div>

          <!-- 内容区 -->
          <div class="relative min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
            <!-- 配置 -->
            <div v-if="activeTab === 'config'" class="mx-auto max-w-3xl space-y-4">
              <p v-if="!plugin.config_schema.length" class="text-sm text-white/50">
                {{ t("settings.plugins.noConfig") }}
              </p>
              <template v-else>
                <div
                  v-for="field in plugin.config_schema"
                  :key="field.key"
                  class="flex flex-col gap-1.5"
                >
                  <label
                    :for="`plugin-config-${field.key}`"
                    class="text-[13px] font-medium text-white/60"
                  >
                    {{ field.label }}
                  </label>
                  <Toggle
                    v-if="field.kind === 'boolean'"
                    :checked="draft[field.key] === true"
                    @change="(v: boolean) => (draft[field.key] = v)"
                  />
                  <input
                    v-else
                    :id="`plugin-config-${field.key}`"
                    class="glass-input"
                    :type="
                      field.kind === 'secret'
                        ? 'password'
                        : field.kind === 'number'
                          ? 'number'
                          : 'text'
                    "
                    :value="draft[field.key] ?? ''"
                    :placeholder="
                      field.kind === 'secret' ? t('settings.plugins.secretKeepHint') : ''
                    "
                    @input="draft[field.key] = ($event.target as HTMLInputElement).value"
                  />
                  <!-- 字段说明：告诉用户这里该填什么、去哪儿拿 -->
                  <p
                    v-if="field.hint"
                    class="text-[11px] leading-relaxed whitespace-pre-line text-white/45"
                  >
                    {{ field.hint }}
                  </p>
                </div>
                <p class="text-[11px] leading-relaxed text-white/40">
                  {{ t("settings.plugins.configApplyHint") }}
                </p>
              </template>
            </div>

            <!-- 携带资源 -->
            <div v-else-if="activeTab === 'resources'" class="mx-auto max-w-3xl space-y-2">
              <div
                v-if="resourcesLoading && !resources.length"
                class="py-6 text-center text-sm text-white/50"
              >
                {{ t("settings.shared.loading") }}
              </div>
              <p v-else-if="!resources.length" class="text-sm text-white/50">
                {{ t("settings.plugins.resourcesEmpty") }}
              </p>
              <template v-else>
                <p class="mb-1 text-[11px] text-white/50">
                  {{ t("settings.plugins.resourcesTitle") }}（{{
                    plugin.resources.map((k) => kindLabel(k as ResourceKind)).join(" / ")
                  }}）
                </p>
                <div
                  v-for="res in resources"
                  :key="res.kind + '/' + res.key"
                  class="flex items-center gap-2 rounded-lg bg-black/10 px-2.5 py-1.5 text-xs"
                >
                  <span
                    class="shrink-0 rounded border border-white/10 bg-white/5 px-1.5 py-0.5 text-[10px] text-white/50"
                  >
                    {{ kindLabel(res.kind) }}
                  </span>
                  <span class="min-w-0 flex-1 truncate text-white/80">{{ res.name }}</span>
                  <span
                    v-if="res.conflict"
                    class="shrink-0 text-[10px] text-amber-300"
                    :title="t('settings.plugins.resourceConflictHint')"
                  >
                    {{ t("settings.plugins.resourceConflict") }}
                  </span>
                  <span v-else-if="res.hidden" class="shrink-0 text-[10px] text-white/40">
                    {{ t("settings.plugins.resourceHidden") }}
                  </span>

                  <template v-if="res.hidden">
                    <button
                      type="button"
                      class="shrink-0 rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-[11px] text-white/70 transition-colors hover:bg-white/10 disabled:opacity-50"
                      @click="restoreResource(res)"
                    >
                      {{ t("settings.plugins.resourceRestore") }}
                    </button>
                  </template>
                  <template v-else>
                    <button
                      v-if="!res.conflict"
                      type="button"
                      class="bg-brand/70 hover:bg-brand shrink-0 rounded-md px-2 py-0.5 text-[11px] text-white transition-colors disabled:opacity-50"
                      :title="t('settings.plugins.resourceKeepHint')"
                      @click="keepResource(res)"
                    >
                      {{ t("settings.plugins.resourceKeep") }}
                    </button>
                    <button
                      type="button"
                      class="shrink-0 rounded-md border border-white/10 bg-white/5 px-2 py-0.5 text-[11px] text-white/70 transition-colors hover:bg-white/10 disabled:opacity-50"
                      @click="hideResource(res)"
                    >
                      {{ t("settings.plugins.resourceHide") }}
                    </button>
                  </template>
                </div>
              </template>
            </div>

            <!-- 详情 -->
            <div v-else class="mx-auto max-w-3xl space-y-6">
              <section v-if="plugin.tools.length">
                <h3 class="mb-2 text-sm font-semibold text-white/80">
                  {{ t("settings.plugins.details.tools") }}
                </h3>
                <div class="flex flex-wrap gap-1.5">
                  <span
                    v-for="tool in plugin.tools"
                    :key="tool"
                    class="rounded-md border border-white/10 bg-white/5 px-2 py-0.5 font-mono text-[11px] text-white/70"
                  >
                    {{ tool }}
                  </span>
                </div>
              </section>

              <section v-if="plugin.env.length">
                <h3 class="mb-2 text-sm font-semibold text-white/80">
                  {{ t("settings.plugins.envHint") }}
                </h3>
                <div
                  v-for="env in plugin.env"
                  :key="env.key"
                  class="flex items-center gap-2 py-0.5"
                >
                  <span class="font-mono text-xs text-white/80">{{ env.key }}</span>
                  <span
                    class="rounded border border-white/10 bg-white/5 px-1.5 py-0.5 text-[10px] text-white/50"
                  >
                    {{ t("settings.plugins.envFromProcess") }}
                  </span>
                </div>
              </section>

              <section v-if="plugin.depends_on.length">
                <h3 class="mb-2 text-sm font-semibold text-white/80">
                  {{ t("settings.plugins.dependsOn") }}
                </h3>
                <div class="flex flex-wrap gap-1.5">
                  <span
                    v-for="dep in plugin.depends_on"
                    :key="dep"
                    class="rounded-md border border-white/10 bg-white/5 px-2 py-0.5 font-mono text-[11px] text-white/70"
                  >
                    {{ dep }}
                  </span>
                </div>
              </section>

              <section v-if="plugin.ws.length">
                <h3 class="mb-2 text-sm font-semibold text-white/80">
                  {{ t("settings.plugins.wsTitle") }}
                </h3>
                <div
                  v-for="conn in plugin.ws"
                  :key="conn.id"
                  class="flex items-center gap-2 py-0.5 text-xs"
                >
                  <span class="h-1.5 w-1.5 shrink-0 rounded-full" :class="wsStateDot(conn.state)" />
                  <span class="font-mono text-white/80">{{ conn.id }}</span>
                  <span class="text-white/40">{{ wsModeLabel(conn.mode) }}</span>
                  <span class="text-white/60">{{ wsStateLabel(conn.state) }}</span>
                </div>
              </section>

              <p
                v-if="
                  !plugin.tools.length &&
                  !plugin.env.length &&
                  !plugin.depends_on.length &&
                  !plugin.ws.length
                "
                class="text-sm text-white/50"
              >
                {{ t("settings.plugins.details.empty") }}
              </p>
            </div>
          </div>
        </div>

        <!-- 操作失败提示：任何 tab 下都可见 -->
        <div
          v-if="error"
          class="border-t border-red-500/20 bg-red-500/10 px-4 py-2 text-xs whitespace-pre-line text-red-200"
        >
          {{ error }}
        </div>

        <!-- 底部 -->
        <div
          class="flex flex-wrap items-center justify-between gap-3 border-t border-white/10 bg-white/5 p-4"
        >
          <button
            :disabled="saving || deleting"
            class="cursor-pointer rounded-[20px] border border-red-400/30 bg-red-500/15 px-4 py-2 text-sm font-medium text-red-200 transition-all duration-200 hover:-translate-y-px hover:bg-red-500/30 hover:shadow-[0_4px_12px_rgba(239,68,68,0.3)] disabled:cursor-not-allowed disabled:opacity-60"
            @click="removePlugin"
          >
            <span class="inline-flex items-center gap-1.5">
              <Trash2 :size="14" />
              {{ t("settings.plugins.delete") }}
            </span>
          </button>

          <div class="flex gap-3">
            <button
              class="cursor-pointer rounded-[20px] border-none bg-white/10 px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:bg-white/20 disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="saving"
              @click="requestClose"
            >
              {{ t("settings.plugins.cancel") }}
            </button>
            <button
              class="cursor-pointer rounded-[20px] border-none bg-[#5e72e4] px-5 py-2 text-sm font-medium text-white transition-all duration-200 hover:enabled:-translate-y-px hover:enabled:bg-[#4a5acf] hover:enabled:shadow-[0_4px_12px_rgba(94,114,228,0.3)] disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="saving || !dirty || !plugin.config_schema.length"
              @click="save"
            >
              <span
                v-if="saving"
                class="mr-2 inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/30 border-t-white"
              ></span>
              {{ t("settings.plugins.saveConfig") }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Trash2 } from "@lucide/vue";
import { Icon, Toggle } from "@/components/base";
import { useDialogStore } from "@/stores/modules/ui/dialog";
import {
  deletePlugin,
  pluginResourceHide,
  pluginResourceKeep,
  pluginResourceRestore,
  pluginResources,
  savePluginConfig,
  type PluginInfo,
  type PluginResourceEntry,
  type ResourceKind,
} from "@/api/services/plugins";
import { kindLabel, wsModeLabel, wsStateDot, wsStateLabel } from "./display";

const props = defineProps<{
  visible: boolean;
  plugin: PluginInfo | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "saved"): void;
  (e: "deleted"): void;
}>();

const { t } = useI18n();
const dialogStore = useDialogStore();

type PluginTab = "config" | "resources" | "details";

/** 表单值：布尔保留为布尔，其余统一用字符串（后端保存时会按声明类型归一化）。 */
type FieldValue = string | boolean;

const activeTab = ref<PluginTab>("config");
const draft = ref<Record<string, FieldValue>>({});
const baseline = ref("");
const saving = ref(false);
const deleting = ref(false);
const error = ref("");
const resources = ref<PluginResourceEntry[]>([]);
const resourcesLoading = ref(false);
/** 已加载过资源的插件 id：切回资源页时不必重复请求。 */
let resourcesLoadedFor: string | null = null;

const tabs = computed(() => {
  const list: { id: PluginTab; label: string }[] = [
    { id: "config", label: t("settings.plugins.tabs.config") },
  ];
  if (props.plugin?.resources.length) {
    list.push({ id: "resources", label: t("settings.plugins.tabs.resources") });
  }
  list.push({ id: "details", label: t("settings.plugins.tabs.details") });
  return list;
});

/** 规范化快照：secret 只在用户填了值时才计入，用来判断是否有未保存改动。 */
const snapshotOf = (plugin: PluginInfo, values: Record<string, FieldValue>): string =>
  JSON.stringify(
    plugin.config_schema.map((field) => [
      field.key,
      field.kind === "secret" ? (values[field.key] as string) || null : (values[field.key] ?? ""),
    ]),
  );

const dirty = computed(() =>
  props.plugin ? snapshotOf(props.plugin, draft.value) !== baseline.value : false,
);

const initDraft = () => {
  const plugin = props.plugin;
  if (!plugin) {
    draft.value = {};
    baseline.value = "";
    return;
  }
  const next: Record<string, FieldValue> = {};
  for (const field of plugin.config_schema) {
    // secret 不回填：留空保存时后端会保留旧值
    if (field.kind === "secret") {
      next[field.key] = "";
      continue;
    }
    const saved = plugin.config?.[field.key];
    const fallback = saved !== undefined && saved !== null ? saved : field.default;
    if (fallback !== undefined && fallback !== null) {
      next[field.key] = typeof fallback === "boolean" ? fallback : String(fallback);
    } else {
      next[field.key] = field.kind === "boolean" ? false : "";
    }
  }
  draft.value = next;
  baseline.value = snapshotOf(plugin, next);
};

const loadResources = async () => {
  const plugin = props.plugin;
  if (!plugin?.resources.length) return;
  const id = plugin.id;
  resourcesLoading.value = true;
  try {
    const list = await pluginResources(id);
    // 切换插件后迟到的响应直接丢掉
    if (props.plugin?.id !== id) return;
    resources.value = list;
    resourcesLoadedFor = id;
  } catch (e) {
    if (props.plugin?.id === id) error.value = String(e);
  } finally {
    if (props.plugin?.id === id) resourcesLoading.value = false;
  }
};

const refreshResources = async () => {
  resourcesLoadedFor = null;
  await loadResources();
};

const resourceKey = (res: PluginResourceEntry) => `${res.kind}/${res.key}`;

const hideResource = async (res: PluginResourceEntry) => {
  const plugin = props.plugin;
  if (!plugin) return;
  try {
    await pluginResourceHide(plugin.id, resourceKey(res));
    await refreshResources();
  } catch (e) {
    error.value = String(e);
  }
};

const restoreResource = async (res: PluginResourceEntry) => {
  const plugin = props.plugin;
  if (!plugin) return;
  try {
    await pluginResourceRestore(plugin.id, resourceKey(res));
    await refreshResources();
  } catch (e) {
    error.value = String(e);
  }
};

const keepResource = async (res: PluginResourceEntry) => {
  const plugin = props.plugin;
  if (!plugin) return;
  const confirmed = await dialogStore.confirm(
    t("settings.plugins.resourceKeepConfirm", {
      name: res.name,
      kind: kindLabel(res.kind),
    }),
  );
  if (!confirmed) return;
  try {
    await pluginResourceKeep(plugin.id, resourceKey(res));
    await refreshResources();
  } catch (e) {
    error.value = String(e);
  }
};

watch(activeTab, (tab) => {
  if (tab === "resources" && props.plugin && resourcesLoadedFor !== props.plugin.id) {
    void loadResources();
  }
});

// 打开（或换一个插件）时重建草稿；列表刷新导致的对象换新不重置，避免冲掉用户输入
watch(
  () => [props.visible, props.plugin?.id] as const,
  ([visible]) => {
    if (!visible) return;
    activeTab.value = "config";
    error.value = "";
    resources.value = [];
    resourcesLoadedFor = null;
    initDraft();
  },
  { immediate: true },
);

const requestClose = async () => {
  if (saving.value) return;
  if (
    dirty.value &&
    !(await dialogStore.confirm(
      t("settings.plugins.discardMessage"),
      t("settings.plugins.discardTitle"),
    ))
  ) {
    return;
  }
  emit("close");
};

const buildPayload = (): Record<string, unknown> => {
  const payload: Record<string, unknown> = {};
  for (const field of props.plugin?.config_schema ?? []) {
    const value = draft.value[field.key];
    // secret 留空 = 保持原值，不放进 payload
    if (field.kind === "secret" && !value) continue;
    payload[field.key] = value;
  }
  return payload;
};

const save = async () => {
  const plugin = props.plugin;
  if (!plugin || saving.value) return;
  saving.value = true;
  error.value = "";
  try {
    await savePluginConfig(plugin.id, buildPayload());
    emit("saved");
  } catch (e) {
    error.value = t("settings.plugins.saveFailed", { error: String(e) });
  } finally {
    saving.value = false;
  }
};

const removePlugin = async () => {
  const plugin = props.plugin;
  if (!plugin || saving.value || deleting.value) return;
  if (!(await dialogStore.confirm(t("settings.plugins.deleteConfirm", { name: plugin.name })))) {
    return;
  }
  deleting.value = true;
  error.value = "";
  try {
    await deletePlugin(plugin.id);
    emit("deleted");
  } catch (e) {
    error.value = String(e);
  } finally {
    deleting.value = false;
  }
};
</script>

<style scoped>
/* 竖排标签栏的细滚动条（Webkit + Firefox） */
.tab-sidebar-scroll {
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.18) transparent;
}
.tab-sidebar-scroll::-webkit-scrollbar {
  width: 6px;
}
.tab-sidebar-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.tab-sidebar-scroll::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.18);
  border-radius: 3px;
}
.tab-sidebar-scroll::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.32);
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}
.modal-enter-active > div,
.modal-leave-active > div {
  transition:
    transform 0.2s ease,
    opacity 0.2s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
.modal-enter-from > div,
.modal-leave-to > div {
  transform: scale(0.95);
  opacity: 0;
}
</style>
