<template>
  <!-- 视图：永久记忆调试（高级设置 → 菜单 → 记忆调试）。只读检视，不写任何状态。 -->
  <div class="flex h-full min-h-0 flex-col md:grid md:grid-cols-[min(28%,240px)_1fr]">
    <!-- 左栏：角色列表 -->
    <nav
      class="border-brand flex shrink-0 flex-col gap-1 overflow-y-auto border-b pb-2 md:border-r md:border-b-0 md:pb-0"
    >
      <div class="px-4 py-2 text-xs text-white/50">{{ $t("settings.memoryDebug.rolesTitle") }}</div>
      <p v-if="roles.length === 0" class="px-4 text-sm text-white/60">
        {{ $t("settings.memoryDebug.noRoles") }}
      </p>
      <button
        v-for="role in roles"
        :key="role.roleId"
        class="mx-2 cursor-pointer rounded-lg px-3 py-2 text-left text-sm transition-colors duration-200"
        :class="
          role.roleId === selectedRoleId
            ? 'bg-brand text-white'
            : 'text-white/70 hover:bg-white/10 hover:text-white'
        "
        @click="selectRole(role.roleId)"
      >
        <span class="block truncate">{{ role.displayName || `角色 ${role.roleId}` }}</span>
        <span class="block text-xs opacity-70">
          #{{ role.roleId }} ·
          {{
            role.runtimePresent
              ? role.enabled
                ? $t("settings.memoryDebug.runtimeOn")
                : $t("settings.memoryDebug.runtimeOff")
              : $t("settings.memoryDebug.runtimeAbsent")
          }}
        </span>
      </button>
    </nav>

    <!-- 右栏：详情 -->
    <section class="min-h-0 flex-1 overflow-y-auto p-4">
      <!-- 工具条 -->
      <div class="mb-4 flex flex-wrap items-center gap-3">
        <h3 class="text-brand text-base font-bold">
          {{ snapshot?.displayName || $t("settings.memoryDebug.title") }}
        </h3>
        <span
          class="rounded-full border px-2 py-0.5 text-xs"
          :class="
            overview?.activeSavePersistent
              ? 'border-white/20 text-white/70'
              : 'border-amber-400/60 text-amber-300'
          "
        >
          {{
            overview?.activeSavePersistent
              ? $t("settings.memoryDebug.saveBound", { id: overview?.activeSaveId })
              : $t("settings.memoryDebug.saveNone")
          }}
        </span>
        <Button type="big" :disabled="loading" @click="refreshAll">
          <RefreshCw :size="16" :class="{ 'animate-spin': loading }" />
          {{ loading ? $t("settings.memoryDebug.refreshing") : $t("settings.memoryDebug.refresh") }}
        </Button>
      </div>

      <p v-if="error" class="mb-4 rounded-lg border border-red-400/40 bg-red-500/10 p-3 text-sm">
        {{ error }}
      </p>
      <p v-else-if="!snapshot" class="text-sm text-white/60">
        {{ $t("settings.memoryDebug.selectRoleHint") }}
      </p>

      <!-- 守卫直接判 runtime（而非 snapshot）：模板里 v-else 无法为 computed 收窄类型 -->
      <template v-else-if="runtime">
        <!-- 压缩状态 -->
        <div class="mb-4 rounded-xl border border-white/10 bg-black/15 p-4">
          <h4 class="mb-1 text-sm font-semibold text-white">
            {{ $t("settings.memoryDebug.triggerTitle") }}
          </h4>
          <p class="mb-3 text-xs leading-5 text-white/60">
            {{ $t("settings.memoryDebug.triggerHint") }}
          </p>

          <div class="mb-1 flex items-baseline justify-between text-sm">
            <span class="text-white/80">
              {{
                $t("settings.memoryDebug.triggerProgress", {
                  visible: runtime.accumulatedVisibleCount,
                  interval: runtime.updateInterval,
                })
              }}
            </span>
            <span class="text-white/60">
              {{
                reached
                  ? $t("settings.memoryDebug.triggerReached")
                  : $t("settings.memoryDebug.triggerRemaining", { n: remaining })
              }}
            </span>
          </div>
          <div class="h-2 w-full overflow-hidden rounded-full bg-white/10">
            <div class="bg-brand h-full" :style="{ width: `${percent}%` }"></div>
          </div>

          <p
            v-if="runtime.pointerOutOfRange"
            class="mt-3 rounded border border-amber-400/50 px-2 py-1 text-xs text-amber-300"
          >
            {{ $t("settings.memoryDebug.pointerOutOfRange") }}
          </p>

          <!-- 状态格子 -->
          <dl class="mt-4 grid grid-cols-2 gap-x-4 gap-y-2 text-xs sm:grid-cols-3">
            <div v-for="item in stateItems" :key="item.label">
              <dt class="text-white/50">{{ item.label }}</dt>
              <dd class="text-white/85">{{ item.value }}</dd>
            </div>
          </dl>

          <p class="mt-3 text-xs text-white/45">
            {{ $t("settings.memoryDebug.runtimeEffective") }}
          </p>
        </div>

        <!-- ① 存储真源 -->
        <div class="mb-3 rounded-xl border border-white/10 bg-black/15 p-4">
          <button
            class="flex w-full cursor-pointer items-center justify-between text-left"
            @click="showStore = !showStore"
          >
            <span>
              <span class="text-sm font-semibold text-white">
                {{ $t("settings.memoryDebug.storeTitle") }}
              </span>
              <span class="ml-2 text-xs text-white/50">
                {{ $t("settings.memoryDebug.storeHint") }}
              </span>
            </span>
            <ChevronDown
              :size="15"
              class="shrink-0 text-gray-400 transition-transform duration-200"
              :class="{ 'rotate-180': showStore }"
            />
          </button>

          <div v-if="showStore" class="mt-3 space-y-3">
            <div v-for="sec in runtime.sections" :key="sec.key">
              <div class="mb-1 flex flex-wrap items-center gap-2 text-xs">
                <span class="text-brand">{{ sectionLabel(sec.key) }}</span>
                <span class="text-white/50">
                  {{
                    sec.limit === 0
                      ? $t("settings.memoryDebug.charsNoLimit", { stored: sec.storedChars })
                      : $t("settings.memoryDebug.charsOf", {
                          stored: sec.storedChars,
                          limit: sec.limit,
                        })
                  }}
                </span>
                <span
                  v-if="sec.truncated"
                  class="rounded border border-amber-400/60 px-1.5 text-amber-300"
                >
                  {{ $t("settings.memoryDebug.truncatedTag") }}
                </span>
              </div>
              <pre
                class="max-h-40 overflow-y-auto rounded bg-black/30 p-2 text-xs break-all whitespace-pre-wrap text-white/80"
                >{{ sectionText(sec.key) }}</pre
              >
            </div>
            <p class="text-xs text-white/45">
              {{
                $t("settings.memoryDebug.metaLine", {
                  pointer: runtime.bank.meta.last_processed_global_idx,
                  updated: runtime.bank.meta.updated_at || "—",
                  schema: runtime.bank.schema_version,
                })
              }}
            </p>
          </div>
        </div>

        <!-- ② 注入视图 -->
        <div class="mb-3 rounded-xl border border-white/10 bg-black/15 p-4">
          <button
            class="flex w-full cursor-pointer items-center justify-between text-left"
            @click="showInjected = !showInjected"
          >
            <span>
              <span class="text-sm font-semibold text-white">
                {{ $t("settings.memoryDebug.injectedTitle") }}
              </span>
              <span class="ml-2 text-xs text-white/50">
                {{ $t("settings.memoryDebug.injectedHint") }}
              </span>
            </span>
            <ChevronDown
              :size="15"
              class="shrink-0 text-gray-400 transition-transform duration-200"
              :class="{ 'rotate-180': showInjected }"
            />
          </button>

          <div v-if="showInjected" class="mt-3 space-y-3">
            <div>
              <p class="mb-1 text-xs text-white/50">
                {{ $t("settings.memoryDebug.injectedSystem") }}
              </p>
              <pre
                class="max-h-60 overflow-y-auto rounded bg-black/30 p-2 text-xs break-all whitespace-pre-wrap text-white/80"
                >{{ runtime.injectedSystemText }}</pre
              >
            </div>
            <div>
              <p class="mb-1 text-xs text-white/50">
                {{ $t("settings.memoryDebug.injectedShortTerm") }}
              </p>
              <pre
                class="max-h-40 overflow-y-auto rounded bg-black/30 p-2 text-xs break-all whitespace-pre-wrap text-white/80"
                >{{
                  runtime.injectedShortTermText || $t("settings.memoryDebug.injectedShortTermEmpty")
                }}</pre
              >
            </div>
          </div>
        </div>

        <!-- ③ 真实上下文 -->
        <div class="rounded-xl border border-white/10 bg-black/15 p-4">
          <button
            class="flex w-full cursor-pointer items-center justify-between text-left"
            @click="showContext = !showContext"
          >
            <span>
              <span class="text-sm font-semibold text-white">
                {{ $t("settings.memoryDebug.contextTitle") }}
              </span>
              <span class="ml-2 text-xs text-white/50">
                {{ $t("settings.memoryDebug.contextHint", { n: context.length }) }}
              </span>
            </span>
            <ChevronDown
              :size="15"
              class="shrink-0 text-gray-400 transition-transform duration-200"
              :class="{ 'rotate-180': showContext }"
            />
          </button>

          <div v-if="showContext" class="mt-3">
            <p v-if="context.length === 0" class="text-sm text-white/60">
              {{ $t("settings.memoryDebug.contextEmpty") }}
            </p>
            <ul v-else class="space-y-2">
              <li
                v-for="(msg, index) in context"
                :key="index"
                class="rounded-lg bg-white/5 px-3 py-2 text-xs"
              >
                <div class="mb-1 flex items-center gap-2">
                  <span class="text-white/40">{{ index }}</span>
                  <span class="font-semibold" :class="roleClass(msg.role)">{{ msg.role }}</span>
                  <span v-if="msg.tool_call_id" class="truncate text-white/40">
                    {{ $t("settings.memoryDebug.toolCallId") }}: {{ msg.tool_call_id }}
                  </span>
                </div>
                <pre
                  v-if="msg.content"
                  class="max-h-60 overflow-y-auto break-all whitespace-pre-wrap text-white/80"
                  >{{ msg.content }}</pre
                >
                <div v-if="msg.tool_calls?.length" class="mt-1">
                  <p class="mb-1 text-white/50">{{ $t("settings.memoryDebug.toolCalls") }}</p>
                  <pre
                    class="max-h-40 overflow-y-auto rounded bg-black/30 p-2 break-all whitespace-pre-wrap text-white/70"
                    >{{ prettyJson(JSON.stringify(msg.tool_calls)) }}</pre
                  >
                </div>
              </li>
            </ul>
          </div>
        </div>
      </template>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onActivated, onMounted, ref, watch } from "vue";
import { Button } from "../../base";
import { ChevronDown, RefreshCw } from "lucide-vue-next";
import { useUIStore } from "@/stores/modules/ui/ui";
import { i18n } from "@/locales";
import {
  getMemoryDebugOverview,
  getRoleMemorySnapshot,
  type GameMemoryBankData,
  type MemoryDebugRole,
  type MemorySectionStat,
  type MemorySystemSnapshot,
  type RoleMemorySnapshot,
} from "@/api/services/memory-debug";

const uiStore = useUIStore();

const overview = ref<Awaited<ReturnType<typeof getMemoryDebugOverview>> | null>(null);
const snapshot = ref<RoleMemorySnapshot | null>(null);
const selectedRoleId = ref<number | null>(null);
const loading = ref(false);
const error = ref("");

const showStore = ref(true);
const showInjected = ref(false);
const showContext = ref(false);

const roles = computed<MemoryDebugRole[]>(() => overview.value?.roles ?? []);
const runtime = computed<MemorySystemSnapshot | null>(() => snapshot.value?.runtime ?? null);
const context = computed(() => snapshot.value?.context ?? []);
const remaining = computed(() =>
  runtime.value
    ? Math.max(0, runtime.value.updateInterval - runtime.value.accumulatedVisibleCount)
    : 0,
);
const reached = computed(() => !!runtime.value && remaining.value === 0);
const percent = computed(() => {
  if (!runtime.value || runtime.value.updateInterval === 0) return 0;
  return Math.min(
    100,
    Math.round((runtime.value.accumulatedVisibleCount / runtime.value.updateInterval) * 100),
  );
});

// 状态格子：全部取运行时生效值
const stateItems = computed(() => {
  const r = runtime.value;
  if (!r) return [];
  const t = (key: string, params?: Record<string, unknown>) =>
    params ? i18n.global.t(key, params) : i18n.global.t(key);
  return [
    {
      label: t("settings.memoryDebug.stEnabled"),
      value: t(r.enabled ? "settings.memoryDebug.enabledOn" : "settings.memoryDebug.enabledOff"),
    },
    { label: t("settings.memoryDebug.stAiName"), value: r.aiName || "—" },
    {
      label: t("settings.memoryDebug.stIsUpdating"),
      value: t(r.isUpdating ? "settings.memoryDebug.yes" : "settings.memoryDebug.no"),
    },
    {
      label: t("settings.memoryDebug.stHasPending"),
      value: t(r.hasPending ? "settings.memoryDebug.yes" : "settings.memoryDebug.no"),
    },
    { label: t("settings.memoryDebug.stFailCount"), value: String(r.failCount) },
    {
      label: t("settings.memoryDebug.stCooldown"),
      value:
        r.cooldownRemainingMs > 0
          ? t("settings.memoryDebug.cooldownValue", {
              s: Math.ceil(r.cooldownRemainingMs / 1000),
            })
          : "—",
    },
    { label: t("settings.memoryDebug.stRecentWindow"), value: String(r.recentWindow) },
    {
      label: t("settings.memoryDebug.stPointer"),
      value: `${r.pointerRaw} → ${r.pointerEffective} / ${r.lineCount}`,
    },
    { label: t("settings.memoryDebug.stHistoryRevision"), value: String(r.historyRevision) },
  ];
});

const sectionLabel = (key: MemorySectionStat["key"]) =>
  i18n.global.t(`settings.memoryDebug.section.${key}`);

const sectionText = (key: MemorySectionStat["key"]) => {
  const data = runtime.value?.bank.data as GameMemoryBankData | undefined;
  return data?.[key] ?? "";
};

const roleClass = (role: string) => {
  switch (role) {
    case "system":
      return "text-amber-300";
    case "user":
      return "text-sky-300";
    case "assistant":
      return "text-emerald-300";
    case "tool":
      return "text-fuchsia-300";
    default:
      return "text-white/70";
  }
};

const prettyJson = (raw: string) => {
  if (!raw) return "—";
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
};

const loadSnapshot = async (roleId: number) => {
  try {
    snapshot.value = await getRoleMemorySnapshot(roleId);
    error.value = "";
  } catch (e) {
    snapshot.value = null;
    error.value = String(e);
  }
};

const selectRole = async (roleId: number) => {
  selectedRoleId.value = roleId;
  await loadSnapshot(roleId);
};

const refreshAll = async () => {
  loading.value = true;
  error.value = "";
  try {
    const data = await getMemoryDebugOverview();
    overview.value = data;
    // currentRoleId 可能为 null，或已不在列表里（角色离场）——
    // 命中才选中，否则退回第一项，避免详情请求留下空白卡片。
    const ids = data.roles.map((r) => r.roleId);
    const preferred =
      data.currentRoleId !== null && ids.includes(data.currentRoleId)
        ? data.currentRoleId
        : (ids[0] ?? null);
    selectedRoleId.value = preferred;
    if (preferred === null) {
      snapshot.value = null;
    } else {
      await loadSnapshot(preferred);
    }
  } catch (e) {
    overview.value = null;
    snapshot.value = null;
    error.value = String(e);
  } finally {
    loading.value = false;
  }
};

onMounted(refreshAll);
// 设置面板是 v-show + KeepAlive：切走再切回不重跑 setup，靠 activated 补一次。
onActivated(refreshAll);
watch(
  () => uiStore.advanceTab,
  (tab) => {
    if (tab === "memory") void refreshAll();
  },
);
</script>
