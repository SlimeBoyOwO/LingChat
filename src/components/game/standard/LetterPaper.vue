<template>
  <Transition
    enter-active-class="transition duration-500 ease-out"
    enter-from-class="translate-x-16 rotate-3 opacity-0"
    leave-active-class="transition duration-300 ease-in"
    leave-to-class="translate-x-16 rotate-3 opacity-0"
  >
    <div
      v-if="letter.visible"
      ref="paperRef"
      class="letter-paper absolute top-20 z-5 flex flex-col"
      :data-paper="letter.paperStyle"
      :class="
        letter.maximized
          ? 'right-[6%] bottom-[26%] left-[6%]'
          : 'right-[5%] bottom-[30%] w-[min(40vw,560px)] max-md:right-4 max-md:left-4 max-md:w-auto'
      "
      :style="resizeStyle"
      @click="menuOpen = false"
      @pointerdown.stop
      @wheel.stop
    >
      <div
        v-if="letter.paperStyle !== 'minimal'"
        class="letter-tape pointer-events-none absolute -top-3 left-1/2 h-6 w-24 -translate-x-1/2 -rotate-2 rounded-sm shadow-sm"
      ></div>
      <template v-if="letter.paperStyle === 'cute'">
        <Leaf
          class="pointer-events-none absolute -top-3 -left-3 h-8 w-8 -rotate-45 fill-pink-300 text-pink-400 opacity-80"
        />
        <Leaf
          class="pointer-events-none absolute right-3 bottom-3 h-9 w-9 rotate-[100deg] fill-pink-200 text-pink-300 opacity-50"
        />
      </template>

      <template v-if="letter.maximized">
        <div
          v-for="handle in RESIZE_HANDLES"
          :key="handle.edge"
          class="absolute z-10 rounded-full transition-colors hover:bg-(--paper-accent)/25"
          :class="handle.class"
          @pointerdown="startResize(handle.edge, $event)"
        ></div>
      </template>

      <header class="flex items-center gap-1 px-4 pt-4 pb-2 text-(--paper-muted) select-none">
        <div class="flex items-center gap-1.5">
          <ScrollText class="h-4 w-4 text-(--paper-accent)" />
          <span class="text-sm font-semibold tracking-widest text-(--paper-ink)">{{
            t("ui.letterPaper.title")
          }}</span>
          <span class="font-serif text-xs tracking-wide italic"
            >· {{ t("ui.letterPaper.subtitle") }}</span
          >
        </div>
        <div class="ml-auto flex items-center gap-0.5">
          <template v-if="letter.current">
            <button
              class="letter-btn"
              :title="t('ui.letterPaper.zoomOut')"
              :disabled="letter.zoom <= LETTER_ZOOM_MIN"
              @click.stop="letter.setZoom(letter.zoom - 0.1)"
            >
              <Minus class="h-4 w-4" />
            </button>
            <span class="w-10 text-center text-xs tabular-nums"
              >{{ Math.round(letter.zoom * 100) }}%</span
            >
            <button
              class="letter-btn"
              :title="t('ui.letterPaper.zoomIn')"
              :disabled="letter.zoom >= LETTER_ZOOM_MAX"
              @click.stop="letter.setZoom(letter.zoom + 0.1)"
            >
              <Plus class="h-4 w-4" />
            </button>
          </template>
          <button
            class="letter-btn max-md:hidden"
            :title="letter.maximized ? t('ui.letterPaper.restore') : t('ui.letterPaper.maximize')"
            @click.stop="letter.maximized = !letter.maximized"
          >
            <Minimize2 v-if="letter.maximized" class="h-4 w-4" />
            <Maximize2 v-else class="h-4 w-4" />
          </button>
          <div class="relative">
            <button
              class="letter-btn"
              :title="t('ui.letterPaper.more')"
              @click.stop="menuOpen = !menuOpen"
            >
              <Ellipsis class="h-4 w-4" />
            </button>
            <div
              v-if="menuOpen"
              class="absolute top-8 right-0 z-20 w-44 overflow-hidden rounded-md border border-(--paper-line) bg-(--paper-bg) py-1 text-sm text-(--paper-ink) shadow-lg"
              @click.stop
            >
              <template v-if="letter.current">
                <button class="letter-menu-item" @click="copyMarkdown">
                  {{ t("ui.letterPaper.copy") }}
                </button>
                <button v-if="!mobile" class="letter-menu-item" @click="revealSource">
                  {{ t("ui.letterPaper.reveal") }}
                </button>
                <button class="letter-menu-item" @click="download">
                  {{ t("ui.letterPaper.download") }}
                </button>
                <div class="my-1 border-t border-(--paper-line)"></div>
              </template>
              <div class="px-3.5 pt-1 pb-0.5 text-xs text-(--paper-muted)">
                {{ t("ui.letterPaper.style") }}
              </div>
              <button
                v-for="style in LETTER_STYLES"
                :key="style"
                class="letter-menu-item flex items-center justify-between"
                @click="pickStyle(style)"
              >
                {{ t(`ui.letterPaper.styles.${style}`) }}
                <Check
                  v-if="letter.paperStyle === style"
                  class="h-3.5 w-3.5 text-(--paper-accent)"
                />
              </button>
            </div>
          </div>
          <button class="letter-btn" :title="t('ui.letterPaper.close')" @click.stop="letter.hide()">
            <X class="h-4 w-4" />
          </button>
        </div>
      </header>

      <div
        ref="scrollRef"
        class="min-h-0 flex-1 overflow-auto overscroll-contain px-7 pb-5 perspective-[1200px]"
      >
        <Transition
          mode="out-in"
          enter-active-class="transition duration-200 ease-out"
          leave-active-class="transition duration-150 ease-in"
          :enter-from-class="
            turnForward
              ? 'translate-x-10 rotate-y-12 opacity-0'
              : '-translate-x-10 -rotate-y-12 opacity-0'
          "
          :leave-to-class="
            turnForward
              ? '-translate-x-10 -rotate-y-12 opacity-0'
              : 'translate-x-10 rotate-y-12 opacity-0'
          "
          @after-leave="scrollToTop"
        >
          <MarkdownText
            v-if="letter.current"
            :key="letter.current.id"
            class="letter-body"
            :style="{ fontSize: `${letter.zoom * 0.95}rem` }"
            :content="letter.current.content"
            :render="renderLetterMarkdown"
          />
          <div
            v-else
            key="empty"
            class="flex h-full flex-col items-center justify-center gap-3 text-center text-(--paper-muted) select-none"
          >
            <Feather class="h-8 w-8 opacity-60" />
            <p class="text-sm tracking-wide">{{ t("ui.letterPaper.empty") }}</p>
          </div>
        </Transition>
      </div>

      <footer
        v-if="letter.pages.length > 1"
        class="flex items-center justify-center gap-3 pb-3 text-sm text-(--paper-muted) select-none"
      >
        <button
          class="letter-btn"
          :title="t('ui.letterPaper.prev')"
          :disabled="letter.index === 0"
          @click.stop="letter.turn(-1)"
        >
          <ChevronLeft class="h-4 w-4" />
        </button>
        <span class="tabular-nums">{{ letter.index + 1 }} / {{ letter.pages.length }}</span>
        <button
          class="letter-btn"
          :title="t('ui.letterPaper.next')"
          :disabled="letter.index >= letter.pages.length - 1"
          @click.stop="letter.turn(1)"
        >
          <ChevronRight class="h-4 w-4" />
        </button>
      </footer>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  Check,
  ChevronLeft,
  ChevronRight,
  Ellipsis,
  Feather,
  Leaf,
  Maximize2,
  Minimize2,
  Minus,
  Plus,
  ScrollText,
  X,
} from "@lucide/vue";
import { MarkdownText } from "@/components/base";
import {
  LETTER_STYLES,
  LETTER_ZOOM_MAX,
  LETTER_ZOOM_MIN,
  type LetterStyle,
  useLetterStore,
} from "@/stores/modules/letter";
import { useUIStore } from "@/stores/modules/ui/ui";
import { renderLetterMarkdown } from "@/utils/letter-markdown";
import { isMobile } from "@/utils/platform";

type Edge = "top" | "right" | "bottom" | "left";

const RESIZE_HANDLES: { edge: Edge; class: string }[] = [
  { edge: "top", class: "inset-x-6 -top-1.5 h-3 cursor-ns-resize" },
  { edge: "bottom", class: "inset-x-6 -bottom-1.5 h-3 cursor-ns-resize" },
  { edge: "left", class: "inset-y-6 -left-1.5 w-3 cursor-ew-resize" },
  { edge: "right", class: "inset-y-6 -right-1.5 w-3 cursor-ew-resize" },
];
const MIN_WIDTH = 320;
const MIN_HEIGHT = 220;

const { t } = useI18n();
const letter = useLetterStore();
const uiStore = useUIStore();
const mobile = isMobile();

const menuOpen = ref(false);
const paperRef = ref<HTMLElement | null>(null);
const scrollRef = ref<HTMLElement | null>(null);
const turnForward = ref(true);

watch(
  () => letter.index,
  (index, old) => {
    turnForward.value = index >= old;
  },
);

function scrollToTop() {
  scrollRef.value?.scrollTo({ top: 0 });
}

// 换页由翻页动画结束时回顶，同一页内容被改写时直接回顶
watch(
  () => [letter.current?.id, letter.current?.content] as const,
  ([id, content], [oldId, oldContent]) => {
    if (id === oldId && content !== oldContent) void nextTick(scrollToTop);
  },
);

// 全屏时拖动四边得到的边距，只在本次全屏内有效
const inset = ref<Record<Edge, number> | null>(null);
watch(
  () => letter.maximized,
  () => {
    inset.value = null;
  },
);

const resizeStyle = computed(() => {
  const value = inset.value;
  if (!letter.maximized || !value) return undefined;
  return {
    top: `${value.top}px`,
    right: `${value.right}px`,
    bottom: `${value.bottom}px`,
    left: `${value.left}px`,
  };
});

function startResize(edge: Edge, event: PointerEvent) {
  const el = paperRef.value;
  const parent = el?.offsetParent as HTMLElement | null;
  if (!el || !parent) return;
  event.preventDefault();

  // 外层可能整体缩放，指针位移要换算回布局像素
  const scale = el.getBoundingClientRect().width / el.offsetWidth || 1;
  const width = parent.clientWidth;
  const height = parent.clientHeight;
  const start = inset.value ?? {
    top: el.offsetTop,
    left: el.offsetLeft,
    right: width - el.offsetLeft - el.offsetWidth,
    bottom: height - el.offsetTop - el.offsetHeight,
  };
  const x0 = event.clientX;
  const y0 = event.clientY;
  const clamp = (value: number, max: number) => Math.max(0, Math.min(value, max));

  const handle = event.currentTarget as HTMLElement;
  handle.setPointerCapture(event.pointerId);

  const move = (e: PointerEvent) => {
    const dx = (e.clientX - x0) / scale;
    const dy = (e.clientY - y0) / scale;
    const next = { ...start };
    if (edge === "left") next.left = clamp(start.left + dx, width - start.right - MIN_WIDTH);
    if (edge === "right") next.right = clamp(start.right - dx, width - start.left - MIN_WIDTH);
    if (edge === "top") next.top = clamp(start.top + dy, height - start.bottom - MIN_HEIGHT);
    if (edge === "bottom") next.bottom = clamp(start.bottom - dy, height - start.top - MIN_HEIGHT);
    inset.value = next;
  };
  const end = () => {
    handle.removeEventListener("pointermove", move);
    handle.removeEventListener("pointerup", end);
    handle.removeEventListener("pointercancel", end);
  };
  handle.addEventListener("pointermove", move);
  handle.addEventListener("pointerup", end);
  handle.addEventListener("pointercancel", end);
}

function pickStyle(style: LetterStyle) {
  letter.paperStyle = style;
  menuOpen.value = false;
}

function fail(e: unknown) {
  uiStore.showError({ title: t("ui.letterPaper.failed"), message: String(e) });
}

async function copyMarkdown() {
  menuOpen.value = false;
  const page = letter.current;
  if (!page) return;
  try {
    await navigator.clipboard.writeText(page.content);
    uiStore.showSuccess({ title: t("ui.letterPaper.copied") });
  } catch (e) {
    fail(e);
  }
}

async function revealSource() {
  menuOpen.value = false;
  const page = letter.current;
  if (!page) return;
  try {
    const path = await invoke<string>("get_letter_path", { id: page.id });
    await revealItemInDir(path);
  } catch (e) {
    fail(e);
  }
}

function suggestedFileName(content: string): string {
  const firstLine = content
    .split("\n")
    .map((line) => line.replace(/^#+/, "").trim())
    .find((line) => line.length > 0);
  const name = (firstLine ?? "")
    .replace(/[\\/:*?"<>|$`]/g, "")
    .slice(0, 40)
    .trim();
  return `${name || t("ui.letterPaper.title")}.md`;
}

async function download() {
  menuOpen.value = false;
  const page = letter.current;
  if (!page) return;
  try {
    const target = await save({
      defaultPath: suggestedFileName(page.content),
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (!target) return;
    await invoke("export_letter", { id: page.id, target });
    uiStore.showSuccess({ title: t("ui.letterPaper.downloaded") });
  } catch (e) {
    fail(e);
  }
}
</script>

<style scoped>
/* 三款信纸共用一套颜色变量，正文、按钮和滚动条都跟着款式走 */
.letter-paper {
  background-color: var(--paper-bg);
  color: var(--paper-ink);
}
.letter-paper[data-paper="classic"] {
  --paper-bg: #fdf8ec;
  --paper-ink: #3b3226;
  --paper-heading: #2c241a;
  --paper-muted: #8a7a63;
  --paper-accent: #b5562f;
  --paper-line: rgba(90, 70, 40, 0.2);
  --paper-tint: rgba(90, 70, 40, 0.08);
  --paper-code: #f6f1e4;
  --paper-thumb: rgba(140, 110, 70, 0.4);
  --paper-tape: rgba(253, 230, 138, 0.6);
  background-image: repeating-linear-gradient(
    transparent 0,
    transparent calc(1.75rem - 1px),
    rgba(120, 150, 190, 0.18) calc(1.75rem - 1px),
    rgba(120, 150, 190, 0.18) 1.75rem
  );
  border-radius: 0.375rem;
  box-shadow: 0 18px 40px rgba(30, 20, 5, 0.35);
}
.letter-paper[data-paper="cute"] {
  --paper-bg: #fff7f6;
  --paper-ink: #5a3d48;
  --paper-heading: #c2416f;
  --paper-muted: #c48a9e;
  --paper-accent: #e0608a;
  --paper-line: rgba(224, 96, 138, 0.25);
  --paper-tint: rgba(244, 143, 177, 0.14);
  --paper-code: #fffafc;
  --paper-thumb: rgba(236, 128, 163, 0.5);
  --paper-tape: repeating-linear-gradient(
    45deg,
    rgba(249, 168, 196, 0.85) 0 6px,
    rgba(255, 255, 255, 0.85) 6px 12px
  );
  background-image:
    repeating-linear-gradient(
      transparent 0,
      transparent calc(2rem - 1px),
      rgba(236, 128, 163, 0.18) calc(2rem - 1px),
      rgba(236, 128, 163, 0.18) 2rem
    ),
    linear-gradient(#ffe6ee, #fffaf6 70%);
  border: 1px solid rgba(236, 128, 163, 0.25);
  border-radius: 1rem;
  box-shadow: 0 14px 34px rgba(190, 80, 120, 0.25);
}
.letter-paper[data-paper="minimal"] {
  --paper-bg: #ffffff;
  --paper-ink: #2b2b2b;
  --paper-heading: #111111;
  --paper-muted: #9a9a9a;
  --paper-accent: #3f6fd8;
  --paper-line: rgba(0, 0, 0, 0.1);
  --paper-tint: rgba(0, 0, 0, 0.05);
  --paper-code: #f6f8fa;
  --paper-thumb: rgba(0, 0, 0, 0.2);
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: 0.5rem;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.18);
}

.letter-tape {
  background: var(--paper-tape);
}

.letter-paper,
.letter-paper :deep(*) {
  scrollbar-width: thin;
  scrollbar-color: var(--paper-thumb) transparent;
}
.letter-paper :deep(::-webkit-scrollbar) {
  width: 6px;
  height: 6px;
}
.letter-paper :deep(::-webkit-scrollbar-thumb) {
  background: var(--paper-thumb);
  border-radius: 9999px;
}
.letter-paper :deep(::-webkit-scrollbar-track) {
  background: transparent;
}

.letter-btn {
  display: flex;
  height: 1.75rem;
  min-width: 1.75rem;
  align-items: center;
  justify-content: center;
  border-radius: 9999px;
  padding: 0 0.25rem;
  transition: background-color 0.15s;
}
.letter-btn:hover:not(:disabled) {
  background-color: var(--paper-tint);
}
.letter-btn:disabled {
  opacity: 0.3;
}

.letter-menu-item {
  width: 100%;
  padding: 0.45rem 0.9rem;
  text-align: left;
}
.letter-menu-item:hover {
  background-color: var(--paper-tint);
}

/* 全局 .markdown-body 是深色主题，这里换成纸面配色，字号随缩放走 em */
.letter-body {
  color: var(--paper-ink);
  line-height: 1.75;
}
.letter-body :deep(h1),
.letter-body :deep(h2),
.letter-body :deep(h3),
.letter-body :deep(h4),
.letter-body :deep(h5),
.letter-body :deep(h6) {
  color: var(--paper-heading);
}
.letter-body :deep(h1) {
  font-size: 1.45em;
  border-bottom-color: var(--paper-line);
}
.letter-body :deep(h2) {
  font-size: 1.25em;
}
.letter-body :deep(h3) {
  font-size: 1.1em;
}
.letter-body :deep(code) {
  background: var(--paper-tint);
  font-size: 0.88em;
}
.letter-body :deep(pre) {
  background: var(--paper-code);
  border-color: var(--paper-line);
}
.letter-body :deep(pre code) {
  background: transparent;
  font-size: 0.85em;
}
.letter-body :deep(blockquote) {
  background: var(--paper-tint);
  color: var(--paper-muted);
}
.letter-body :deep(th),
.letter-body :deep(td) {
  border-color: var(--paper-line);
}
.letter-body :deep(th) {
  background: var(--paper-tint);
}
.letter-body :deep(a) {
  color: var(--paper-accent);
}
.letter-body :deep(hr) {
  border-color: var(--paper-line);
}
.letter-body :deep(img) {
  max-width: 100%;
  border-radius: 4px;
}
.letter-body :deep(.letter-svg) {
  display: flex;
  justify-content: center;
  margin: 0.8em 0;
}
.letter-body :deep(.letter-svg img) {
  width: 100%;
  max-height: 60vh;
  object-fit: contain;
}
.letter-body :deep(.letter-math) {
  overflow-x: auto;
  margin: 0.6em 0;
}
</style>
