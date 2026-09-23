<script setup lang="ts">
/**
 * 章节预览浮窗：只读展示某个剧本已落盘的章节与事件时间线。
 *
 * 用文件系统事实当真相（章节列表来自 `Chapters/` 扫描），因此旧剧本一样能看。
 * 渲染直接复用编辑器的 `ChapterTimeline`（只读模式），保证样式与「章节流程」一致。
 * 刻意不碰 store.chapter —— 那是编辑器正在编辑的章节，带自动保存防抖，误用会写盘。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { readChapter, readScript } from "@/api/services/script-editor";
import type { ChapterSummary, ScriptDetail, ScriptEventData } from "@/api/services/script-editor";
import ChapterTimeline from "@/components/script-editor/flow/ChapterTimeline.vue";

const props = defineProps<{ open: boolean; scriptKey: string | null }>();
const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();

const detail = ref<ScriptDetail | null>(null);
const error = ref("");
const loading = ref(false);
const currentId = ref("");
const events = ref<ScriptEventData[]>([]);
const opening = ref(false);

const chapters = computed<ChapterSummary[]>(() => detail.value?.chapters ?? []);

const title = computed(() => {
  const pkg = detail.value?.package;
  return pkg?.scriptName?.trim() || pkg?.folderName || props.scriptKey || "";
});

const roleNameMap = computed(
  () => new Map((detail.value?.characters ?? []).map((c) => [c.roleKey, c.aiName])),
);

/** MAIN 的展示名：与编辑器 getter 同口径（绑定角色优先，其次剧本里的玩家名）。 */
const mainRoleName = computed(() => {
  const d = detail.value;
  if (!d) return "";
  const bound = d.package.boundCharacterFolder;
  if (bound) {
    const c = d.characters.find((x) => x.folder === bound);
    if (c?.aiName) return c.aiName;
  }
  const settings = d.storyConfig?.script_settings as Record<string, unknown> | undefined;
  const userName = settings?.user_name;
  if (typeof userName === "string" && userName.trim()) return userName.trim();
  return t("scriptEditor.fieldRow.mainRole");
});

async function openChapter(id: string) {
  const key = props.scriptKey;
  if (!key || opening.value || id === currentId.value) return;
  opening.value = true;
  try {
    const content = await readChapter(key, id);
    events.value = content.events;
    currentId.value = id;
  } catch (e) {
    error.value = String(e);
  } finally {
    opening.value = false;
  }
}

/** 每次打开重新加载：会话期间编辑器可能改过章节 */
async function load() {
  detail.value = null;
  error.value = "";
  events.value = [];
  currentId.value = "";
  const key = props.scriptKey;
  if (!key) return;
  loading.value = true;
  try {
    detail.value = await readScript(key);
    const first = chapters.value[0];
    if (first) await openChapter(first.id);
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.open,
  (open) => {
    if (open) void load();
  },
  { immediate: true },
);
</script>

<template>
  <!-- Teleport 到 #app：浮层若挂在 body 下会脱离整体缩放作用域（见 PreviewStage 注释） -->
  <Teleport to="#app">
    <Transition
      enter-active-class="transition-opacity duration-200 ease"
      leave-active-class="transition-opacity duration-200 ease"
      enter-from-class="opacity-0"
      leave-to-class="opacity-0"
    >
      <div
        v-if="open"
        class="modal-mask fixed inset-0 z-[9999] flex items-center justify-center bg-black/55 p-4 backdrop-blur-md"
        @click.self="emit('close')"
      >
        <div
          class="flex h-[min(78dvh,760px)] w-[min(1080px,94vw)] flex-col overflow-hidden rounded-xl border border-white/12.5 bg-[rgba(12,20,30,0.86)] shadow-[0_8px_32px_rgba(0,0,0,0.45),inset_0_1px_1px_rgba(255,255,255,0.06)] backdrop-blur-lg backdrop-saturate-[1.4]"
        >
          <div class="border-brand flex shrink-0 items-center gap-2 border-b-2 px-4.5 pt-3.5 pb-2">
            <h4 class="truncate font-semibold text-white">
              {{
                scriptKey
                  ? t("scriptEditor.agentScriptPreview.title", { name: title })
                  : t("scriptEditor.agentScriptPreview.noScriptTitle")
              }}
            </h4>
            <span v-if="scriptKey" class="text-brand/70 shrink-0 font-mono text-[0.66rem]"
              >📕 {{ scriptKey }}</span
            >
            <span
              class="ml-auto shrink-0 rounded-full border border-white/10 bg-white/5 px-2 py-px text-[0.64rem] text-white/45"
            >
              {{ t("scriptEditor.agentScriptPreview.readOnly") }}
            </span>
            <button
              class="hover:text-brand shrink-0 cursor-pointer px-1 text-white/50 transition-all duration-300 hover:rotate-90"
              @click="emit('close')"
            >
              ✕
            </button>
          </div>

          <div v-if="!scriptKey" class="px-4 py-8 text-center text-[0.82rem] text-white/50">
            {{ t("scriptEditor.agentScriptPreview.noScript") }}
          </div>
          <div v-else-if="loading" class="px-4 py-6 text-[0.8rem] text-white/45">
            {{ t("scriptEditor.agentScriptPreview.loading") }}
          </div>
          <div v-else-if="error" class="px-4 py-6 text-[0.8rem] text-red-300">
            {{ t("scriptEditor.agentScriptPreview.loadFailed", { error }) }}
          </div>
          <div v-else class="flex min-h-0 flex-1">
            <!-- 左：只读事件时间线（直接落在玻璃面板上，与「章节流程」同观感） -->
            <div class="min-h-0 flex-1 overflow-y-auto px-4.5 py-3">
              <ChapterTimeline
                v-if="events.length"
                readonly
                :events="events"
                :role-name-map="roleNameMap"
                :main-role-name="mainRoleName"
              />
              <p v-else class="text-[0.8rem] text-white/45">
                {{ t("scriptEditor.agentScriptPreview.emptyChapter") }}
              </p>
            </div>

            <!-- 右：已落盘章节列表（面板内层的通用表面：白 5% + 白色 10% 描边） -->
            <aside
              class="flex min-h-0 w-[212px] shrink-0 flex-col border-l border-white/10 bg-white/[0.03]"
            >
              <div class="px-3 pt-3 pb-2 text-[0.7rem] tracking-[0.5px] text-white/38">
                {{ t("scriptEditor.agentScriptPreview.chapters", { count: chapters.length }) }}
              </div>
              <div class="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-2 pb-3">
                <button
                  v-for="c in chapters"
                  :key="c.id"
                  class="rounded-lg border px-2.5 py-2 text-left transition-all duration-150"
                  :class="
                    c.id === currentId
                      ? 'border-brand/60 bg-brand/12'
                      : 'hover:border-brand/40 border-white/10 bg-white/5 hover:bg-white/10'
                  "
                  @click="openChapter(c.id)"
                >
                  <div class="flex items-center gap-1.5">
                    <span
                      class="border-brand/40 text-brand shrink-0 rounded border px-[5px] py-px font-mono text-[0.66rem]"
                      >{{ c.id }}</span
                    >
                    <span class="min-w-0 flex-1 truncate text-[0.76rem] text-white/80">{{
                      c.name || c.id
                    }}</span>
                  </div>
                  <div class="mt-0.5 text-[0.64rem] text-white/35">
                    {{ t("scriptEditor.agentScriptPreview.eventCount", { count: c.eventCount }) }}
                  </div>
                </button>
                <p v-if="!chapters.length" class="px-1.5 text-[0.74rem] text-white/40">
                  {{ t("scriptEditor.agentScriptPreview.empty") }}
                </p>
              </div>
            </aside>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
