<template>
  <div
    class="game-dialog relative z-2 flex w-full scrollbar-thin [scrollbar-color:var(--accent-color)_transparent] justify-center p-3.75 transition-all duration-200 ease-[cubic-bezier(0.25,0.46,0.45,0.94)] before:pointer-events-none before:absolute before:-top-10 before:right-0 before:left-0 before:h-10 before:bg-linear-to-b before:from-transparent before:via-[rgba(0,14,39,0.3)] before:to-[rgba(0,14,39,0.6)] before:content-['']"
    :class="{
      [`z-[-1]! overflow-hidden opacity-0 duration-500! ease-linear before:opacity-0 before:duration-1000!`]:
        isHidden,
      'max-h-[40dvh]': !uiStore.isNarrowScreen,
    }"
    :style="dialogWrapperStyle"
    @wheel="handleWheelHistory"
  >
    <div :style="{ width: containerWidth + '%' }" class="relative">
      <div class="overflow-y-auto">
        <!-- 标题栏 -->
        <div class="mb-2 flex items-baseline">
          <!-- 角色名称 + 副标题：两者一起切换，旧标题向上滑出、新标题从上方滑入 -->
          <Transition name="title-slide">
            <div
              :key="titleSubtitleKey"
              class="flex items-baseline"
              :class="{ 'min-w-0': uiStore.isNarrowScreen }"
            >
              <div
                class="mr-3.75 font-[inherit] text-2xl font-bold text-shadow-[inherit]"
                :class="{
                  'min-w-0 overflow-hidden text-ellipsis whitespace-nowrap': uiStore.isNarrowScreen,
                }"
                :style="{ color: dialogTextColorValue }"
              >
                <div id="character">{{ uiStore.showCharacterTitle }}</div>
              </div>
              <div
                v-show="!uiStore.isNarrowScreen"
                class="font-[inherit] text-xl font-bold text-[#6eb4ff] text-shadow-[inherit]"
              >
                <div id="character-sub">{{ uiStore.showCharacterSubtitle }}</div>
              </div>
            </div>
          </Transition>

          <!-- 情绪标签 -->
          <div
            class="relative mx-4 shrink-0 font-[inherit] text-xl font-bold whitespace-nowrap text-[#ff77dd] text-shadow-[inherit]"
          >
            <Transition name="emotion-slide">
              <div id="character-emotion" :key="uiStore.showCharacterEmotion" class="inline-block">
                {{ uiStore.showCharacterEmotion }}
              </div>
            </Transition>
          </div>

          <!-- 操作按钮组配置 -->
          <div class="ml-auto flex min-w-0 items-baseline">
            <!-- 桌面端：直接显示所有操作按钮 -->
            <template v-if="!isMobile">
              <!-- 操作按钮组 -->
              <div
                class="custom-scroll overflow-x-auto"
                :class="uiStore.isNarrowScreen ? 'min-w-0 flex-1' : 'shrink-0'"
              >
                <div class="flex whitespace-nowrap">
                  <Button
                    type="nav"
                    icon="background"
                    :title="$t('game.dialog.sceneSettings')"
                    @click="openSceneSettings"
                  ></Button>
                  <!--
                  <Button
                    type="nav"
                    icon="hand"
                    :title="$t('game.dialog.touchMode')"
                    @click="toggleTouchMode"
                    @contextmenu.prevent="exitTouchMode"
                  ></Button>
                  -->
                  <Button
                    type="nav"
                    icon="history"
                    :title="$t('game.dialog.history')"
                    @click="openHistory"
                  ></Button>

                  <!-- 语音输入按钮（auto_listen 开启时变为关闭开关） -->
                  <Button
                    type="nav"
                    :icon="micIconName"
                    :title="micTitle"
                    :class="{
                      'animate-asr-breathe text-blue-500': asrInput.phase.value === 'recording',
                    }"
                    :disabled="!micEnabled"
                    @click="toggleRecording"
                  ></Button>

                  <ScreenshotButton @start="startScreenshot" @clear="clearScreenshot" />

                  <Button
                    type="nav"
                    icon="close"
                    :title="$t('game.dialog.closeDialog')"
                    @click="removeDialog"
                  ></Button>
                </div>
              </div>
            </template>

            <!-- 移动端：箭头折叠按钮 + 关闭按钮 -->
            <div v-if="isMobile" class="flex items-baseline gap-1">
              <button
                class="mobile-toggle-btn"
                :class="{ 'is-open': showMobileMenu }"
                :title="$t('game.dialog.moreActions')"
                @click="showMobileMenu = !showMobileMenu"
              >
                ▲
              </button>
              <Button
                type="nav"
                icon="close"
                :title="$t('game.dialog.closeDialog')"
                @click="removeDialog"
              ></Button>
            </div>
          </div>
        </div>

        <!-- 移动端：折叠菜单下拉面板 -->
        <Transition name="mobile-menu">
          <div v-if="isMobile && showMobileMenu" class="mobile-menu-dropdown">
            <div class="custom-scroll flex gap-1 overflow-x-auto pb-1 whitespace-nowrap">
              <Button
                type="nav"
                icon="background"
                :title="$t('game.dialog.sceneSettings')"
                @click="onMobileMenuAction(openSceneSettings)"
              ></Button>
              <Button
                type="nav"
                icon="hand"
                :title="$t('game.dialog.touchMode')"
                @click="onMobileMenuAction(toggleTouchMode)"
                @contextmenu.prevent="exitTouchMode"
              ></Button>
              <Button
                type="nav"
                icon="history"
                :title="$t('game.dialog.history')"
                @click="onMobileMenuAction(openHistory)"
              ></Button>
              <Button
                type="nav"
                :icon="micIconName"
                :title="micTitle"
                :class="{
                  'animate-asr-breathe text-blue-500': asrInput.phase.value === 'recording',
                }"
                :disabled="!micEnabled"
                @click="onMobileMenuAction(toggleRecording)"
              ></Button>
              <ScreenshotButton
                @start="onMobileMenuAction(startScreenshot)"
                @clear="onMobileMenuAction(clearScreenshot)"
              />
            </div>
          </div>
        </Transition>

        <!-- 分割线：青蓝色发光线条，亮段从左向右流动（同源桌宠外框 sweep-glow-ring） -->
        <div class="dialog-divider-glow my-1.5"></div>

        <!-- 输入区 -->
        <div
          class="my-1.25 flex min-h-10 w-full resize-none flex-col border-none bg-transparent text-xl font-bold whitespace-pre-line text-white transition-all duration-300 outline-none"
        >
          <!-- AI 回复显示区（仅回应状态可见；标准/内联模式共用，逐字符淡入+上浮）。
               两子容器：台词区白字 + 动作区灰字，颜色由容器决定，字符只负责动画 span -->
          <div
            v-show="currentStatus === 'responding'"
            ref="inlineDisplayRef"
            tabindex="0"
            class="response-display my-1.25 max-h-[50dvh] min-h-30 flex-1 resize-none overflow-y-auto border-none bg-transparent font-[inherit] text-xl font-bold break-all whitespace-pre-line outline-none text-shadow-[inherit]"
            @keydown.enter.exact.prevent="sendOrContinue"
            @click="sendOrContinue"
          >
            <!-- 台词区：白字（内联模式台词 / 标准模式正文） -->
            <div ref="dialogueLineRef" class="whitespace-pre-line text-white"></div>
            <!-- 动作区：灰字；标准模式两段式动作阶段为斜体小字 -->
            <div
              ref="motionLineRef"
              class="whitespace-pre-line text-[#9ca3af]"
              :class="{ 'text-base italic': isShowingMotionText }"
            ></div>
          </div>

          <!-- 输入框 textarea（非回应状态：思考/输入/展示阶段） -->
          <textarea
            v-show="currentStatus !== 'responding'"
            id="inputMessage"
            ref="textareaRef"
            class="my-1.25 max-h-[50dvh] min-h-30 flex-1 resize-none border-none bg-transparent font-[inherit] text-[max(1.25rem,16px)] font-bold transition-all duration-300 outline-none text-shadow-[inherit] placeholder:text-white/50 placeholder:shadow-none"
            :placeholder="placeholderText"
            v-model="inputMessage"
            @keydown.enter.exact.prevent="sendOrContinue"
            :readonly="!isInputEnabled"
          ></textarea>
        </div>
      </div>
      <!-- 发送按钮（内层右侧外部） -->
      <button
        id="sendButton"
        class="absolute right-0 bottom-0 translate-x-full cursor-pointer rounded-[5px] border-none bg-transparent px-2 py-2 font-[inherit] text-sm font-bold text-[#04bcff] transition-all duration-300 text-shadow-[inherit] hover:bg-transparent hover:text-[rgba(136,255,251,0.827)] disabled:cursor-not-allowed disabled:bg-[#333] disabled:opacity-70"
        :disabled="isSending"
        @click="sendOrContinue"
      >
        ▼
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useTypeWriter } from "../../../composables/ui/useTypeWriter";
import { setMobileMenuOpen, useAsrInput } from "../../../composables/useAsrInput";
import { useChatInput } from "../../../composables/chat/useChatInput";
import { useDialogAdvance } from "../../../composables/chat/useDialogAdvance";
import { useDialogStatus } from "../../../composables/chat/useDialogStatus";
import { useDialogAppearance } from "../../../composables/useDialogAppearance";
import { useMicControl } from "../../../composables/useMicControl";
import { useScreenshot } from "../../../composables/useScreenshot";
import { dialogueMerge } from "../../../core/events/dialogue-merge";
import { eventQueue } from "../../../core/events/event-queue";
import { useGameStore } from "../../../stores/modules/game";
import { useSettingsStore } from "../../../stores/modules/settings";
import { useDialogStore } from "../../../stores/modules/ui/dialog";
import { useUIStore } from "../../../stores/modules/ui/ui";
import { charRevealCharHtml } from "../../../utils/typewriter/charHtml";
import { createCharRevealWriter } from "../../../utils/typewriter/charReveal";
import { TypeWriter } from "../../../utils/typewriter/TypeWriter";
import { Button } from "../../base";
import ScreenshotButton from "./ScreenshotButton.vue";

const { t } = useI18n();
const isShowingMotionText = ref(false);
const textareaRef = ref<HTMLTextAreaElement | null>(null);
const inlineDisplayRef = ref<HTMLDivElement | null>(null);
const gameStore = useGameStore();
const uiStore = useUIStore();
const dialogStore = useDialogStore();
const settingsStore = useSettingsStore();

// Dialog appearance managed by composable: useDialogAppearance
const { isHidden, hide, dialogWrapperStyle, dialogTextColorValue, handleWheelHistory } =
  useDialogAppearance({
    openHistory: () => {
      uiStore.toggleSettings(true);
      uiStore.setSettingsTab("history");
    },
  });

// 移动端按钮折叠状态（但是基于长宽比判断）
const isMobile = ref(uiStore.aspectRatio <= 1);
const showMobileMenu = ref(false);
// 同步给 ASR 模块：移动端菜单展开时禁用语音输入（§1.5）
watch(showMobileMenu, (open) => setMobileMenuOpen(open));

// 当前游戏状态（模板 v-show 判定回复显示区 / 输入框）
const currentStatus = computed(() => gameStore.currentStatus);

// 标题栏（角色名 + 副标题）切换 key：任一变化时整体一起滑出/滑入
const titleSubtitleKey = computed(
  () => `${uiStore.showCharacterTitle}|${uiStore.showCharacterSubtitle}`,
);

// 语音输入：useAsrInput 统一两种触发源（mic 按钮 / 自动监听），
// 替换上游的 Web Speech API 实现（状态为模块级单例，GameRolesStage 等共享）
const asrInput = useAsrInput();

// 三层麦克风语义（功能开关 / 手动录音 / 总闸禁用）—— 与桌宠共用同一实现
const { micEnabled, micTitle, micIconName, toggleRecording } = useMicControl();

// 截图状态改用共享 composable（桌宠早已在用），不再各自维护一套 listen + refs。
// 失败提示由调用方提供：桌宠侧静默，主界面弹 alert。
const {
  hasScreenshot,
  init: initScreenshot,
  destroy: destroyScreenshot,
  start: startScreenshotRaw,
  clear: clearScreenshot,
} = useScreenshot();

const startScreenshot = () =>
  startScreenshotRaw({
    onError: () => {
      void dialogStore.alert(t("game.dialog.screenshotFailed"));
    },
  });

// 响应式容器宽度（窄屏判断从 uiStore 读取）
const containerWidth = ref(60);

const updateContainerWidth = () => {
  containerWidth.value = Math.max(60, uiStore.aspectRatio > 1 ? 70 : 90);
  isMobile.value = uiStore.aspectRatio <= 1;
  if (!isMobile.value) showMobileMenu.value = false;
};

const openSceneSettings = () => {
  uiStore.toggleSettings(true);
  uiStore.setSettingsTab("background");
};

// 移动端菜单操作：执行动作后自动收起菜单
const onMobileMenuAction = (action: () => void) => {
  action();
  showMobileMenu.value = false;
};
const currentDisplayedText = ref("");
const dialogueLineRef = ref<HTMLDivElement | null>(null);
const motionLineRef = ref<HTMLDivElement | null>(null);

// 台词合并显示的分段模型（唯一事实来源）：台词/动作按序排列，颜色由容器 CSS 决定
// （台词区白、动作区灰），普通台词自带换行不再干扰分色。
type DisplaySegment = { kind: "dialogue" | "motion"; text: string };
let segments: DisplaySegment[] = [];

// 把一句台词构造成分段：内联模式台词+动作两段都进，标准模式只进台词段
// （动作由两段式单独处理，见 continueDialog）。
function buildSegments(line: string): DisplaySegment[] {
  const segs: DisplaySegment[] = [{ kind: "dialogue", text: line }];
  if (settingsStore.text.inlineMotionText && uiStore.showCharacterMotionText) {
    segs.push({ kind: "motion", text: uiStore.showCharacterMotionText });
  }
  return segs;
}

// charReveal 只负责「打字内容」：内联模式=台词段（动作段由独立动作打字机负责）；
// 标准模式=全部段（动作由 continueDialog Phase 2 走 charReveal 打字）。台词段间空格已在 append 时烙进段文本。
function typedText(segs: DisplaySegment[] = segments): string {
  if (settingsStore.text.inlineMotionText) {
    return segs
      .filter((s) => s.kind === "dialogue")
      .map((s) => s.text)
      .join("");
  }
  return segs.map((s) => s.text).join("");
}

// 动作区第二打字机（仅内联模式）：动作文本独立逐字打字，保留打字机效果。
// 单独一个 TypeWriter + charReveal，不混进台词打字机流——replace 模式「旧动作被
// 清掉」使文本非单调变化，单流 charReveal 的 prev 前缀去重无法处理。append 用
// append() 接续（| 分隔），replace 用 stop + start 清空重打。
// soundUrls=[] 避免动作打字重复播对话音效。
const motionReveal = createCharRevealWriter({
  charHtml: charRevealCharHtml,
});
let motionWriter: TypeWriter | null = null;
function ensureMotionWriter(): TypeWriter | null {
  if (!motionLineRef.value) return null;
  if (!motionWriter) {
    // 动作打字完成无需额外处理（不参与合并判定，台词 isTyping 已覆盖）
    motionWriter = new TypeWriter(motionLineRef.value, undefined, [], motionReveal.writeFn);
  }
  return motionWriter;
}
function startMotionTyping(text: string, speed?: number) {
  ensureMotionWriter()?.start(text, speed);
}
function appendMotionTyping(text: string) {
  ensureMotionWriter()?.append(text);
}
function stopMotionTyping() {
  motionWriter?.stop();
  motionWriter?.clear();
}

// 链式合并：刚追加的这行（appendedLine，仅聊天文本）若仍够短、且队头下一条是
// 同角色短句回复，就继续武装——使 MainChat 在这行展示完成后自动推进下一条也走
// 追加路径。解决快速连发时 i+2/i+3 在 i+1 处理前就入队、被 addEvent 的队列守卫
// 挡住、导致最多只融合两句的问题。
function rearmNextMerge(appendedLine: string) {
  if (!settingsStore.text.inlineMotionText || settingsStore.text.mergeLineThreshold <= 0) {
    return;
  }
  const next = eventQueue.peek();
  if (!next || next.type !== "reply") return;
  if (next.roleId !== gameStore.currentInteractRoleId) return;
  if (dialogueMerge.mergedLength + next.message.length > settingsStore.text.mergeLineThreshold) {
    // console.log(
    //   "原来的台词长度是:",
    //   dialogueMerge.mergedLength,
    //   "新台词长度是:",
    //   next.message.length,
    //   "超过阈值",
    //   settingsStore.text.mergeLineThreshold,
    //   "，不合并"
    // );
    return;
  }

  dialogueMerge.armed = true;
  dialogueMerge.armedRoleId = next.roleId;
}

// 逐字符淡入+上浮渲染器。颜色不再由字符决定，改由 route 把字符插到对应容器；
// append 续打时下标在整个累积文本上连续，route 按全局下标累计偏移定位。
const charReveal = createCharRevealWriter({
  charHtml: charRevealCharHtml,
  route: (index) => {
    // 内联模式：打字内容只含台词段，一律进台词区；动作区由独立动作打字机负责
    if (settingsStore.text.inlineMotionText) return dialogueLineRef.value;
    let offset = 0;
    for (const seg of segments) {
      if (index < offset + seg.text.length) {
        return seg.kind === "motion" ? motionLineRef.value : dialogueLineRef.value;
      }
      offset += seg.text.length;
    }
    return dialogueLineRef.value;
  },
  // 清空子容器内容、保留容器结构——清外层会把子容器节点销毁，模板 ref 指向
  // 脱离文档的旧节点，之后字符全插进不可见处。
  // 内联模式动作区由独立动作打字机管理，这里只清台词区（台词 typewriter 首个
  // tick 也会触发一次 clear，若清动作区会把动作打字机刚渲染的内容抹掉）；
  // 标准模式动作区走 charReveal 打字，仍需清空（continueDialog Phase 2 前）。
  clear: () => {
    if (dialogueLineRef.value) dialogueLineRef.value.innerHTML = "";
    if (!settingsStore.text.inlineMotionText && motionLineRef.value) {
      motionLineRef.value.innerHTML = "";
    }
  },
});

// 清空回复显示区并重置渲染器增量状态（新台词 / 两段式动作阶段切换前调用）
function resetResponseDisplay() {
  if (inlineDisplayRef.value) charReveal.clear(inlineDisplayRef.value);
  // 内联模式动作区由动作打字机管理：一并停止清空（标准模式动作走 charReveal，上面的 clear 已清掉）
  if (settingsStore.text.inlineMotionText) stopMotionTyping();
}

// 立即把当前台词写入显示元素（不经过打字动画；供挂载恢复使用）
function renderLineInstant(line: string) {
  currentDisplayedText.value = line;
  segments = buildSegments(line);
  if (inlineDisplayRef.value) {
    charReveal.renderInstant(inlineDisplayRef.value, typedText());
    // 内联模式动作区瞬时恢复（重挂载不打字）
    if (settingsStore.text.inlineMotionText) {
      stopMotionTyping();
      const motionText = uiStore.showCharacterMotionText;
      if (motionText && motionLineRef.value) {
        motionReveal.renderInstant(motionLineRef.value, motionText);
      } else if (motionLineRef.value) {
        motionLineRef.value.innerHTML = "";
      }
    }
  }
}

// 回复显示区 TypeWriter（标准/内联模式共用；逐字符渲染由 charReveal 负责；
// appendTyping 用于台词合并续打）
const { startTyping, stopTyping, isTyping, finishTyping, appendTyping } = useTypeWriter(
  inlineDisplayRef,
  (text) => {
    currentDisplayedText.value = text;
  },
  charReveal.writeFn,
);

// 输入框文本/发送/ASR 接线 —— 与桌宠 ChatInput 共用同一实现。
// 别名回 inputMessage，模板绑定不变。
const {
  text: inputMessage,
  isSending,
  send,
} = useChatInput({
  noModelTitleKey: "game.dialog.noModelTitle",
  noModelMessageKey: "game.dialog.noModelMessage",
});

const emit = defineEmits(["player-continued", "dialog-proceed"]);

const openHistory = () => {
  uiStore.toggleSettings(true);
  uiStore.setSettingsTab("history");
};

const handleRightClick = (e: MouseEvent) => {
  if (gameStore.command === "touch") {
    e.preventDefault();
    exitTouchMode();
  }
};

const handleDialogShow = (e: MouseEvent) => {
  if (isHidden.value) {
    e.preventDefault();
    isHidden.value = false;
  }
};

const toggleTouchMode = () => {
  if (gameStore.command === "touch") {
    exitTouchMode();
  } else {
    document.body.style.cursor = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' class='lucide lucide-hand-icon lucide-hand'%3E%3Cpath d='M18 11V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2'/%3E%3Cpath d='M14 10V4a2 2 0 0 0-2-2a2 2 0 0 0-2 2v2'/%3E%3Cpath d='M10 10.5V6a2 2 0 0 0-2-2a2 2 0 0 0-2 2v8'/%3E%3Cpath d='M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.86-5.99-2.34l-3.6-3.6a2 2 0 0 1 2.83-2.82L7 15'/%3E%3C/svg%3E") 0 0, auto`;
    gameStore.command = "touch";
    document.addEventListener("contextmenu", handleRightClick);
  }
};

const exitTouchMode = () => {
  document.body.style.cursor = "default";
  gameStore.command = null;
  document.removeEventListener("contextmenu", handleRightClick);
};

// 状态判定与 input 可用性来自共享实现；主界面独有的副作用在 onStatusChange 里补齐
const { placeholderState, isInputEnabled } = useDialogStatus({
  onStatusChange: (status) => {
    // 离开回应状态（input/presenting/error 等）→ 取消未消费的合并武装
    if (status !== "responding") dialogueMerge.armed = false;
    if (status === "input") {
      uiStore.showCharacterTitle = gameStore.userName;
      uiStore.showCharacterSubtitle = gameStore.userSubtitle;
    } else if (status === "presenting") {
      uiStore.showCharacterTitle = "";
      uiStore.showCharacterSubtitle = "";
      uiStore.showCharacterEmotion = "";
      uiStore.showCharacterLine = "";
    }
  },
});

// 占位符：状态判定来自共享的 useDialogStatus，这里只负责主界面自己的文案
const placeholderText = computed(() => {
  const s = placeholderState.value;
  switch (s.kind) {
    case "recording":
      // 流式模式 partial 已实时写入输入框，此占位仅兜底非流式
      return t("game.dialog.listening");
    case "hint":
      return s.hint || t("game.dialog.inputPlaceholder");
    case "thinking":
      return s.length > 0
        ? `${s.message}${t("game.dialog.thinkingDepth", { count: s.length })}`
        : s.message;
    case "waiting":
      return t("game.dialog.waitingResponse");
    case "responding":
    case "presenting":
      return "";
    default:
      return t("game.dialog.inputPlaceholder");
  }
});

watch([() => uiStore.showCharacterLine, () => gameStore.currentStatus], ([newLine, newStatus]) => {
  if (newLine && newLine !== "" && newStatus === "responding") {
    // 输入框清空由 useChatInput 的同源 watch 负责
    currentDisplayedText.value = "";

    // 合并续打：i+1 到达时 event-queue 已武装（i 仍打字/播音频），MainChat 在 i
    // 展示完成后自动推进队列，这里对 i+1 走追加路径——不重置显示区、续打新增文本。
    // 同一容器里已有内容时，新段前加空格隔离，避免两句黏在一起。
    if (dialogueMerge.armed) {
      dialogueMerge.armed = false;
      const newSegs: DisplaySegment[] = [];
      const appendMerged = (kind: DisplaySegment["kind"], text: string, separator: string) => {
        const hasPrior = segments.some((s) => s.kind === kind);
        newSegs.push({ kind, text: hasPrior ? separator + text : text });
      };
      appendMerged("dialogue", newLine, " ");
      // 动作段记账 + 动作区续打（独立于台词打字机）。hadPriorMotion 必须在
      // segments.push 之前算——push 后新动作段已在 segments 里，会误判成已有前序动作。
      const hadPriorMotion = settingsStore.text.inlineMotionText
        ? segments.some((s) => s.kind === "motion")
        : false;
      if (settingsStore.text.inlineMotionText) {
        const motionText = uiStore.showCharacterMotionText;
        if (settingsStore.text.mergeMotionMode === "replace") {
          // 独立显示模式：清掉旧动作段，只保留本次动作（无动作则动作区清空）
          segments = segments.filter((s) => s.kind !== "motion");
          if (motionText) newSegs.push({ kind: "motion", text: motionText });
          // 清空旧动作并重打本次动作（无动作则清空动作区）
          stopMotionTyping();
          if (motionText) startMotionTyping(motionText, uiStore.typeWriterSpeed);
        } else if (motionText) {
          // 接在后面显示：动作段之间用 | 分隔，动作区接续打字
          appendMerged("motion", motionText, " | ");
          if (hadPriorMotion) appendMotionTyping(" | " + motionText);
          else startMotionTyping(motionText, uiStore.typeWriterSpeed);
        }
      }
      segments.push(...newSegs);
      appendTyping(typedText(newSegs));
      // 链式合并：刚追加的这行若仍满足合并条件、且队头下一条也是同角色短句，
      // 继续武装，让 MainChat 在本行展示完成后再次自动续打下一条——突破只融合两句的限制。
      dialogueMerge.mergedLength += newLine.length;
      rearmNextMerge(newLine);
    } else {
      // 全新台词：重置显示区 + 从 0 打字（台词与动作区并行逐字打字）
      isShowingMotionText.value = false;
      segments = buildSegments(newLine);
      dialogueMerge.mergedLength = newLine.length;
      resetResponseDisplay();
      startTyping(typedText(), uiStore.typeWriterSpeed);
      if (settingsStore.text.inlineMotionText && uiStore.showCharacterMotionText) {
        startMotionTyping(uiStore.showCharacterMotionText, uiStore.typeWriterSpeed);
      }
      rearmNextMerge(newLine);
    }
  } else if (newStatus === "input") {
    stopTyping();
    stopMotionTyping();
    dialogueMerge.mergedLength = 0;
    isShowingMotionText.value = false;
    currentDisplayedText.value = "";
    segments = [];
  }
});

// 同步打字状态给合并判定（event-queue.addEvent 在 i+1 到达时读取）
watch(isTyping, (t) => {
  dialogueMerge.isTyping = t;
});

// 回复 div 可见时自动聚焦，确保 Enter 键能推进对话（textarea 隐藏后这是唯一 Enter 入口）
watch(currentStatus, (status) => {
  if (status === "responding") {
    // setTimeout 确保 v-show 已生效、DOM 已渲染
    setTimeout(() => inlineDisplayRef.value?.focus(), 0);
  }
});

onMounted(async () => {
  // 模式切换重挂载：立即从 store 恢复当前台词（不重播打字动画）
  const restoreLine = uiStore.showCharacterLine;
  if (restoreLine && restoreLine !== "" && gameStore.currentStatus === "responding") {
    renderLineInstant(restoreLine);
  }

  document.addEventListener("contextmenu", handleDialogShow);
  // asr-text / asr-send 监听与输入框桥由 useChatInput 统一注册
  // 初始化容器宽度
  updateContainerWidth();
  // 监听窗口大小变化
  window.addEventListener("resize", updateContainerWidth);

  initScreenshot();
});

onUnmounted(() => {
  // 卸载时清掉打字状态：避免返回主界面后首条回复被当成「续打合并」
  dialogueMerge.isTyping = false;
  // 动作打字机停止并释放（否则 setTimeout 循环可能继续跑）
  motionWriter?.destroy();
  motionWriter = null;
  document.removeEventListener("contextmenu", handleDialogShow);
  window.removeEventListener("resize", updateContainerWidth);
  destroyScreenshot();
});

function sendOrContinue() {
  if (gameStore.currentStatus === "input") {
    send();
  } else if (gameStore.currentStatus === "responding") {
    continueDialog(true);
  }
}

// 推进状态机 —— 与桌宠 DialogueBox 共用；主界面多一层两段式动作文本
const { continueDialog } = useDialogAdvance({
  isTyping,
  finishTyping,
  motion: {
    isShowingMotionText,
    showMotionLine: (motionText) => {
      resetResponseDisplay();
      segments = [{ kind: "motion", text: motionText }];
      startTyping(motionText, uiStore.typeWriterSpeed);
    },
  },
  onProceed: ({ isPlayerTrigger }) => {
    if (isPlayerTrigger) emit("player-continued");
    emit("dialog-proceed");
  },
});

function removeDialog(_e: Event) {
  hide();
}

// ── 对话框外观（响应 settings store） ──
// Dialog appearance logic extracted to composable: useDialogAppearance

defineExpose({
  continueDialog,
  isTyping,
});
</script>

<style scoped>
/* AI 回复显示区：标准/内联模式共用。颜色由两个子容器决定
     （台词区白、动作区灰），这里的灰色仅作 fallback */
.response-display {
  color: #9ca3af;
}

/* 分割线：青蓝色微光点缀，亮段沿线条从左向右流动。
 * 视觉同源桌宠外框（青色 rgba(34,211,238)），但只保留微弱点缀。
 * 用 background-position 驱动而非 transform/子元素：背景只绘制在元素盒内，
 * 不会撑宽布局、也不会触发父级 overflow-x 滚动。 */
.dialog-divider-glow {
  height: 1px;
  border-radius: 9999px;
  background-color: rgba(34, 211, 238, 0.12);
  background-image: linear-gradient(
    90deg,
    transparent 0%,
    rgba(34, 211, 238, 0.12) 30%,
    rgba(34, 211, 238, 0.55) 50%,
    rgba(34, 211, 238, 0.12) 70%,
    transparent 100%
  );
  background-size: 30% 100%;
  background-repeat: no-repeat;
  background-position: -30% 0;
  box-shadow: 0 0 2px rgba(110, 187, 199, 0.01);
  animation: dialog-divider-flow 3s linear infinite;
}
@keyframes dialog-divider-flow {
  from {
    background-position: -30% 0;
  }
  to {
    background-position: 130% 0;
  }
}

/* 兼容 Firefox */
.custom-scroll {
  scrollbar-width: thin;
}

/* 兼容 Chrome / Edge / Safari */
.custom-scroll::-webkit-scrollbar {
  width: 6px; /* 纵向滚动条宽度 */
  height: 6px; /* 横向滚动条高度（你这个是 overflow-x，主要控制这个） */
}

/* 移动端折叠按钮 — 与右侧 nav 关闭按钮等大 */
.mobile-toggle-btn {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: white;
  border-radius: 8px;
  padding: 10px 14px;
  cursor: pointer;
  font-size: 16px;
  line-height: 1;
  transition: all 0.25s ease;
  margin: 0 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 38px;
}
.mobile-toggle-btn:hover {
  background: rgba(255, 255, 255, 0.2);
  color: var(--accent-color, #6eb4ff);
}
.mobile-toggle-btn:active {
  transform: scale(0.92);
}
.mobile-toggle-btn > span,
.mobile-toggle-btn {
  transition: transform 0.25s ease;
}
.mobile-toggle-btn.is-open {
  transform: rotate(180deg);
  background: rgba(255, 255, 255, 0.18);
  color: var(--accent-color, #6eb4ff);
  border-color: var(--accent-color, #6eb4ff);
}

/* 移动端下拉菜单 */
.mobile-menu-dropdown {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding: 8px 4px 4px;
  margin-top: 2px;
  border-top: 1px solid rgba(255, 255, 255, 0.12);
  background: rgba(0, 14, 39, 0.5);
  border-radius: 0 0 8px 8px;
  width: 100%; /* 确保占满整个对话框宽度 */
}

/* Vue Transition: 移动端菜单展开/收起 */
.mobile-menu-enter-active {
  animation: menu-slide-down 0.2s ease-out;
}
.mobile-menu-leave-active {
  animation: menu-slide-down 0.15s ease-in reverse;
}
@keyframes menu-slide-down {
  from {
    opacity: 0;
    max-height: 0;
    padding-top: 0;
    padding-bottom: 0;
    margin-top: 0;
    border-top-width: 0;
  }
  to {
    opacity: 1;
    max-height: 200px;
    padding-top: 8px;
    padding-bottom: 4px;
    margin-top: 2px;
    border-top-width: 1px;
  }
}

/* 情绪标签切换：上一个情绪向左滑出，下一个情绪从右侧滑入（推挤效果） */
.emotion-slide-enter-active,
.emotion-slide-leave-active {
  transition:
    transform 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94),
    opacity 0.3s ease;
}
/* 离开中的旧情绪脱离文档流，覆盖在新情绪上方向左滑出，
 * 容器宽度由新情绪决定，避免标题栏按钮被临时撑开 */
.emotion-slide-leave-active {
  position: absolute;
  left: 0;
  top: 0;
}
.emotion-slide-enter-from {
  transform: translateX(100%);
  opacity: 0;
}
.emotion-slide-leave-to {
  transform: translateX(-100%);
  opacity: 0;
}

/* 标题栏（角色名 + 副标题）切换：整体从上方滑出/滑入（与情绪标签的左右滑动区分） */
.title-slide-enter-active,
.title-slide-leave-active {
  transition:
    transform 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94),
    opacity 0.3s ease;
}
/* 离开中的旧标题脱离文档流，向上滑出，避免撑动标题栏布局 */
.title-slide-leave-active {
  position: absolute;
  left: 0;
  top: 0;
}
.title-slide-enter-from {
  transform: translateY(-100%);
  opacity: 0;
}
.title-slide-leave-to {
  transform: translateY(-100%);
  opacity: 0;
}
</style>

<style>
/* 底部 Home 指示器安全区：对话框本体铺到屏幕底（其半透明底盖住背景条带），
     仅内容底部让出 env() 高度，输入框不被 Home 指示器遮挡（桌面/Android 桌面 env=0） */
.game-dialog {
  padding-bottom: calc(15px + var(--safe-area-inset-bottom, 0px));
}
/* 逐字符淡入+上浮动画的 @keyframes 已移入 src/assets/styles/dialogue-text.css（全局引入） */
</style>
