<template>
  <div ref="rootEl" class="relative">
    <!-- 触发条：只显示当前那盏灯叫什么，点开才给全量信息 -->
    <button
      ref="triggerEl"
      @click="toggle"
      class="flex w-full items-center justify-between gap-2 rounded-xl border border-white/15 bg-black/40 px-3 py-2 text-left transition-colors hover:border-amber-400/50"
    >
      <span
        class="min-w-0 flex-1 truncate text-sm"
        :class="currentLabel ? 'text-white' : 'text-white/40'"
      >
        {{ currentLabel || noneLabel }}
      </span>
      <svg
        class="h-4 w-4 shrink-0 text-white/40 transition-transform"
        :class="open ? 'rotate-180' : ''"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
      >
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
      </svg>
    </button>

    <!-- 选项列表挂到 body：这两处宿主（设置页、场景弹窗）都在 overflow-y-auto 里，
         就地展开会被裁掉半截，下面的预设点不到。 -->
    <Teleport to="body">
      <div
        v-if="open"
        ref="panelEl"
        :style="panelStyle"
        class="z-[10001] flex min-w-[19rem] flex-col overflow-hidden rounded-xl border border-white/15 bg-slate-900/95 shadow-2xl backdrop-blur-2xl"
      >
        <div class="shrink-0 border-b border-white/10 p-2">
          <input
            ref="searchEl"
            v-model="q"
            :placeholder="$t('settings.background.lighting.select.searchPlaceholder')"
            class="w-full rounded-lg border border-white/10 bg-black/40 px-2.5 py-1.5 text-xs text-white placeholder-white/30 focus:border-amber-400/60 focus:outline-none"
          />
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto p-1.5">
          <!-- 不用预设：交回场景自己的灯 / 不套用默认光影 -->
          <div
            @click="pick(null)"
            :class="{ 'bg-amber-400/15': !selectedId }"
            class="flex cursor-pointer items-center gap-2 rounded-lg px-2.5 py-2 transition-colors hover:bg-white/10"
          >
            <span class="text-xs font-bold text-white/80">{{ noneLabel }}</span>
          </div>

          <template v-for="grp in groups" :key="grp.key">
            <div
              v-if="grp.items.length"
              class="px-2.5 pt-2 pb-1 text-[10px] font-bold tracking-wider text-white/35 uppercase"
            >
              {{ $t(`settings.background.lighting.select.${grp.key}Group`) }}
            </div>
            <div
              v-for="p in grp.items"
              :key="p.id"
              @click="pick(p)"
              :class="{ 'bg-amber-400/15': selectedId === p.id }"
              class="group/row flex cursor-pointer items-start gap-2 rounded-lg px-2.5 py-2 transition-colors hover:bg-white/10"
            >
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-1.5">
                  <span class="text-xs font-bold text-white/90">{{ presetNameOf(p) }}</span>
                  <span
                    v-if="p.custom"
                    class="rounded-full bg-amber-400/20 px-1.5 py-0.5 text-[10px] text-amber-200/90"
                    >{{ $t("settings.background.lighting.custom.badge") }}</span
                  >
                </div>
                <!-- 描述常驻：悬停才出的 tooltip 看不见也读不全 -->
                <div class="mt-0.5 text-[11px] leading-snug break-words text-white/45">
                  {{ presetDescriptionOf(p) }}
                </div>
                <div class="mt-1 flex flex-wrap gap-1">
                  <span
                    v-for="m in presetMoodOf(p).slice(0, 4)"
                    :key="m"
                    class="rounded-full bg-white/10 px-1.5 py-0.5 text-[10px] text-white/50"
                    >{{ m }}</span
                  >
                </div>
              </div>
              <div class="flex shrink-0 gap-2 opacity-0 group-hover/row:opacity-100">
                <span
                  @click.stop="openEdit(p)"
                  class="text-[11px] text-white/50 transition-colors hover:text-amber-300"
                  >{{ $t("settings.background.lighting.custom.edit") }}</span
                >
                <span
                  v-if="p.custom"
                  @click.stop="remove(p)"
                  class="text-[11px] text-white/50 transition-colors hover:text-red-300"
                  >{{ $t("settings.background.lighting.custom.delete") }}</span
                >
              </div>
            </div>
          </template>

          <div
            v-if="groups[0].items.length + groups[1].items.length === 0"
            class="px-2.5 py-3 text-center text-xs text-white/35"
          >
            {{ $t("settings.background.lighting.select.empty") }}
          </div>
        </div>

        <div class="shrink-0 border-t border-white/10 p-1.5">
          <div
            @click="openCreate"
            class="cursor-pointer rounded-lg px-2.5 py-2 text-xs font-bold text-amber-200/90 transition-colors hover:bg-amber-400/10"
          >
            {{ $t("settings.background.lighting.select.newPreset") }}
          </div>
        </div>
      </div>
    </Teleport>

    <LightingEditorModal
      :show="editorShow"
      :editing="editingPreset"
      @close="editorShow = false"
      @saved="onSaved"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * 光影预设下拉。以前是一大片卡片网格，19 个内置预设加上「我的预设」越堆越高，
 * 场景编辑器里根本放不下；这里收成一条下拉，描述常驻在每一行下面。
 *
 * 只负责「选哪盏灯」，不碰任何状态：选完把预设原样抛给父组件，套到全局默认还是
 * 灌进场景草稿由父组件决定 —— 预设与场景之间是复制参数，不记引用，
 * 所以「引用 + 手工微调」谁覆盖谁这种问题不会出现在这里。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { CSSProperties } from "vue";
import { useI18n } from "vue-i18n";
import LightingEditorModal from "./LightingEditorModal.vue";
import type { LightingPreset } from "../../../api/services/lighting";
import { clearLighting } from "../../../api/services/lighting";
import { useLightingStore } from "../../../stores/modules/lighting";
import { useDialogStore } from "../../../stores/modules/ui/dialog";
import {
  presetDescriptionOf,
  presetMoodOf,
  presetNameOf,
  presetSearchTextOf,
} from "../../../locales/lighting-i18n";

defineProps<{
  /** 触发条上显示的名字；空串显示 noneLabel */
  currentLabel: string;
  /** 「不使用预设」那一行与触发条空值时的文案（两处语义不同，交给父组件给） */
  noneLabel: string;
  /** 高亮用；不传就只按 currentLabel 显示 */
  selectedId?: string;
}>();

const emit = defineEmits<{
  picked: [preset: LightingPreset | null];
  saved: [preset: LightingPreset];
}>();

const { t } = useI18n();
const lightingStore = useLightingStore();
const dialogStore = useDialogStore();

const rootEl = ref<HTMLElement | null>(null);
const triggerEl = ref<HTMLElement | null>(null);
const panelEl = ref<HTMLElement | null>(null);
const searchEl = ref<HTMLInputElement | null>(null);
const open = ref(false);
const q = ref("");
const editorShow = ref(false);
const editingPreset = ref<LightingPreset | null>(null);
const panelStyle = ref<CSSProperties>({});

/** 视口里放不下就朝上开，别把列表裁成看不见的一截。 */
function reposition() {
  const rect = triggerEl.value?.getBoundingClientRect();
  if (!rect) return;
  const gap = 6;
  const margin = 12;
  const below = window.innerHeight - rect.bottom - gap - margin;
  const above = rect.top - gap - margin;
  const upward = below < 200 && above > below;
  panelStyle.value = {
    position: "fixed",
    left: `${rect.left}px`,
    width: `${Math.max(rect.width, 304)}px`,
    maxHeight: `${Math.max(180, upward ? above : below)}px`,
    ...(upward
      ? { bottom: `${window.innerHeight - rect.top + gap}px` }
      : { top: `${rect.bottom + gap}px` }),
  };
}

const groups = computed(() => {
  const kw = q.value.trim().toLowerCase();
  const hit = (p: LightingPreset) => !kw || presetSearchTextOf(p).includes(kw);
  const all = lightingStore.presets.filter(hit);
  return [
    { key: "custom", items: all.filter((p) => p.custom) },
    { key: "builtin", items: all.filter((p) => !p.custom) },
  ];
});

function toggle() {
  open.value = !open.value;
  if (open.value) {
    reposition();
    void nextTick(() => searchEl.value?.focus());
  }
}

function close() {
  open.value = false;
}

function pick(preset: LightingPreset | null) {
  emit("picked", preset);
  close();
}

function openCreate() {
  editingPreset.value = null;
  editorShow.value = true;
  close();
}

function openEdit(p: LightingPreset) {
  editingPreset.value = p;
  editorShow.value = true;
  close();
}

/**
 * 编辑器自己会刷新预设表；这里只负责把存好的那条抛给父组件去套用。
 *
 * 保存后必须把实时预览留下的运行时覆盖清掉：它压在所有灯光之上，留着就等于
 * 「刚才那盏灯」永远赖在画面上，之后场景自己的灯再也上不去。套哪一盏由父组件决定。
 */
async function onSaved(id: string) {
  editorShow.value = false;
  const saved = lightingStore.presets.find((p) => p.id === id);
  editingPreset.value = null;
  try {
    await clearLighting();
  } catch (e) {
    console.error("[Lighting] 取消预览灯光失败:", e);
  }
  if (saved) emit("saved", saved);
}

async function remove(p: LightingPreset) {
  const ok = await dialogStore.confirm(
    t("settings.background.lighting.custom.deleteConfirm", { name: presetNameOf(p) }),
  );
  if (!ok) return;
  try {
    await lightingStore.removePreset(p.id);
  } catch (e) {
    console.error("[Lighting] 删除自建预设失败:", e);
    dialogStore.alert(t("settings.background.lighting.custom.deleteFailed", { msg: String(e) }));
  }
}

function onDocClick(e: MouseEvent) {
  if (!open.value) return;
  const target = e.target as Node;
  if (rootEl.value?.contains(target) || panelEl.value?.contains(target)) return;
  close();
}

function onDocKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && open.value) close();
}

onMounted(() => {
  document.addEventListener("click", onDocClick, true);
  document.addEventListener("keydown", onDocKeydown);
  void lightingStore.ensurePresets();
});

onBeforeUnmount(() => {
  document.removeEventListener("click", onDocClick, true);
  document.removeEventListener("keydown", onDocKeydown);
  window.removeEventListener("scroll", reposition, true);
  window.removeEventListener("resize", reposition);
});

// 列表挂到 body 之后不跟着宿主滚动，所以一滚动就得重算位置。
watch(open, (isOpen) => {
  if (isOpen) {
    window.addEventListener("scroll", reposition, true);
    window.addEventListener("resize", reposition);
  } else {
    window.removeEventListener("scroll", reposition, true);
    window.removeEventListener("resize", reposition);
  }
});
</script>
