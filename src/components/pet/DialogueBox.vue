<template>
  <div
    class="absolute z-30 flex cursor-pointer"
    :class="[horizontal ? 'w-[85%] flex-col' : 'inset-x-0 justify-center px-2', frameClass]"
    :style="frameStyle"
    @click="emit('advance')"
  >
    <div
      ref="bubbleRef"
      class="hover-up relative rounded-[calc(20px*var(--pet-ui-scale,1))] border border-white/10 bg-neutral-950/50 px-[calc(18px*var(--pet-ui-scale,1))] py-[calc(6px*var(--pet-ui-scale,1))] text-white backdrop-blur-xl backdrop-saturate-200 transition-all duration-300 [text-shadow:0_1px_4px_rgba(0,0,0,0.5)] hover:scale-[1.02] hover:border-white/20 hover:bg-neutral-950/65"
      :class="[horizontal ? 'w-full' : 'w-[85%]', animClass]"
      :style="{ maxHeight: `${maxHeight}px` }"
    >
      <div class="relative overflow-hidden">
        <Transition name="emotion-slide">
          <div
            v-if="emotion"
            :key="emotion"
            class="mb-0.5 inline-block max-w-full truncate text-[calc(12px*var(--pet-ui-scale,1))] font-semibold tracking-wider text-cyan-400 italic drop-shadow-[0_1px_4px_rgba(0,176,255,0.5)]"
          >
            {{ emotion }}
          </div>
        </Transition>
      </div>

      <div
        ref="textRef"
        class="dialog-text-lock overflow-y-auto pb-[0.4em] text-[calc(15px*var(--pet-ui-scale,1))] leading-snug font-medium break-all whitespace-pre-line [text-shadow:0_0_3px_rgba(0,0,0,0.9),0_1px_4px_rgba(0,0,0,0.5)] [&::-webkit-scrollbar]:hidden"
      ></div>

      <div class="absolute h-0 w-0 drop-shadow-md" :class="tailOuterClass"></div>
      <div class="absolute h-0 w-0" :class="tailInnerClass"></div>
    </div>

    <div
      class="absolute inset-x-0 flex justify-center"
      :class="top ? 'top-full mt-1' : 'bottom-full mb-1'"
      :style="{ maxHeight: 'var(--notify-h)' }"
    >
      <PetNotification />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { BubbleAlign, BubbleSide } from "./bubbleMirror";
import { useTypeWriter } from "@/composables/ui/useTypeWriter";
import { createCharRevealWriter } from "@/utils/typewriter/charReveal";
import { charRevealCharHtml } from "@/utils/typewriter/charHtml";
import PetNotification from "./PetNotification.vue";

const props = defineProps<{
  visible: boolean;
  line: string;
  /** 台词序号，由宠物窗随镜像一起发下来；drained 时原样回传，用于区分是哪一句 */
  lineId: number;
  emotion?: string;
  speed?: number;
  instant?: boolean;
  maxHeight: number;
  side?: BubbleSide;
  align?: BubbleAlign;
  alignInset?: number;
}>();

const emit = defineEmits<{ advance: []; drained: [lineId: number] }>();

const side = computed<BubbleSide>(() => props.side ?? "above");
const horizontal = computed(() => side.value === "left" || side.value === "right");
const top = computed(() => (horizontal.value ? props.align !== "bottom" : side.value === "below"));

const frameClass = computed(() => [
  ...(horizontal.value
    ? [side.value === "left" ? "right-(--tail) items-end" : "left-(--tail) items-start"]
    : [top.value ? "top-(--tail) items-start" : "bottom-(--tail) items-end"]),
  props.visible ? "" : "pointer-events-none",
]);

const ALIGN_INSET_TRANSITION_MS = 120;

const frameStyle = computed(() => {
  if (!horizontal.value) return undefined;
  const inset = Math.max(0, props.alignInset ?? 0);
  const transition = `top ${ALIGN_INSET_TRANSITION_MS}ms ease-out, bottom ${ALIGN_INSET_TRANSITION_MS}ms ease-out`;
  return top.value ? { top: `${inset}px`, transition } : { bottom: `${inset}px`, transition };
});

const tailOuterClass = computed(() => {
  if (side.value === "left") {
    return "-right-2.5 top-1/2 -translate-y-1/2 border-t-10 border-b-10 border-r-white/10 border-t-transparent border-b-transparent";
  }
  if (side.value === "right") {
    return "-left-2.5 top-1/2 -translate-y-1/2 border-t-10 border-b-10 border-l-white/10 border-t-transparent border-b-transparent";
  }
  return top.value
    ? "-top-2.5 left-1/2 -translate-x-1/2 border-r-10 border-l-10 border-b-white/10 border-r-transparent border-l-transparent"
    : "-bottom-2.5 left-1/2 -translate-x-1/2 border-r-10 border-l-10 border-t-white/10 border-r-transparent border-l-transparent";
});

const tailInnerClass = computed(() => {
  if (side.value === "left") {
    return "-right-2 top-1/2 -translate-y-1/2 border-t-8 border-b-8 border-r-white/8 border-t-transparent border-b-transparent";
  }
  if (side.value === "right") {
    return "-left-2 top-1/2 -translate-y-1/2 border-t-8 border-b-8 border-l-white/8 border-t-transparent border-b-transparent";
  }
  return top.value
    ? "-top-2 left-1/2 -translate-x-1/2 border-r-8 border-l-8 border-b-white/8 border-r-transparent border-l-transparent"
    : "-bottom-2 left-1/2 -translate-x-1/2 border-r-8 border-l-8 border-t-white/8 border-r-transparent border-l-transparent";
});

const hasShown = ref(false);
watch(
  () => props.visible,
  (visible) => {
    if (visible) hasShown.value = true;
  },
  { immediate: true },
);

const animClass = computed(() => {
  if (props.visible) return `enter-${side.value}`;
  if (!hasShown.value) return "bubble-hidden";
  return `leave-${side.value}`;
});

const textRef = ref<HTMLElement | null>(null);
const bubbleRef = ref<HTMLElement | null>(null);

const charReveal = createCharRevealWriter({ charHtml: charRevealCharHtml });

const { startTyping, stopTyping, finishTyping, isTyping } = useTypeWriter(
  textRef,
  undefined,
  charReveal.writeFn,
);

const displayEmpty = ref(true);
const shownLine = ref<string | null>(null);

let renderToken = 0;

const applyTextHeight = (line: string) => {
  const el = textRef.value;
  if (!el) return;
  if (!line.trim()) {
    el.style.height = "0px";
    return;
  }
  const clone = el.cloneNode(false) as HTMLDivElement;
  clone.style.cssText = `position:fixed;left:-9999px;top:0;visibility:hidden;height:auto;overflow:visible;width:${el.clientWidth}px`;
  el.parentElement?.appendChild(clone);
  charReveal.renderInstant(clone, line);
  const measured = clone.offsetHeight;
  clone.remove();
  charReveal.reset();
  el.style.height = `${Math.min(measured, props.maxHeight)}px`;
};

const clearDisplay = () => {
  stopTyping();
  if (textRef.value) {
    textRef.value.innerHTML = "";
    textRef.value.style.height = "0px";
  }
  charReveal.reset();
  displayEmpty.value = true;
  shownLine.value = null;
};

const render = async (line: string, instant: boolean) => {
  const token = ++renderToken;
  // 和 token 一起捕获：完成信号必须属于本次渲染的那一句，emit 时再读 props.lineId 会串号
  const id = props.lineId;

  stopTyping();
  if (textRef.value) {
    textRef.value.innerHTML = "";
    textRef.value.style.height = "0px";
    void textRef.value.offsetHeight;
  }
  charReveal.reset();
  applyTextHeight(line);

  if (instant) {
    if (textRef.value) charReveal.renderInstant(textRef.value, line);
    displayEmpty.value = false;
    shownLine.value = line;
    emit("drained", id);
    return;
  }

  await startTyping(line, props.speed);
  if (token !== renderToken) return;
  displayEmpty.value = false;
  shownLine.value = line;
  // 自然打完或被 finishTyping 补全（TypeWriter.finish 会收口 start 的 promise）；过期渲染已被 token 挡掉
  if (!isTyping.value) emit("drained", id);
};

watch(
  () => [props.visible, props.line, props.instant] as const,
  ([visible, line, instant], prev) => {
    // 台词清空才丢弃显示内容。drained 只代表「这一句完整显示过」，
    // 隐藏（状态切走）不是结束，更不能上报——否则宠物窗会把它当成能推进的信号。
    if (!line) {
      renderToken++;
      clearDisplay();
      return;
    }
    // 只是隐藏：显示内容与进度原样留着，视觉效果由 CSS 淡出负责
    if (!visible) return;

    const sameLine = shownLine.value === line;
    const stillShown = prev?.[0] === true && !displayEmpty.value;

    if (stillShown && sameLine) return;

    const restoring = prev?.[0] === false && sameLine;
    void render(line, instant || restoring);
  },
  { immediate: true, flush: "post" },
);

defineExpose({
  isTyping,
  finishTyping,
  bubbleRef,
});
</script>

<style scoped>
.bubble-hidden {
  opacity: 0;
}

.enter-above {
  animation: bubble-in-above 300ms cubic-bezier(0, 0, 0.2, 1);
}

.leave-above {
  animation: bubble-out-above 300ms cubic-bezier(0, 0, 0.2, 1) forwards;
}

@keyframes bubble-in-above {
  from {
    opacity: 0;
    transform: translateY(6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@keyframes bubble-out-above {
  from {
    opacity: 1;
    transform: none;
  }
  to {
    opacity: 0;
    transform: translateY(4px) scale(0.98);
  }
}

.enter-below {
  animation: bubble-in-below 300ms cubic-bezier(0, 0, 0.2, 1);
}

.leave-below {
  animation: bubble-out-below 300ms cubic-bezier(0, 0, 0.2, 1) forwards;
}

@keyframes bubble-in-below {
  from {
    opacity: 0;
    transform: translateY(-6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@keyframes bubble-out-below {
  from {
    opacity: 1;
    transform: none;
  }
  to {
    opacity: 0;
    transform: translateY(-4px) scale(0.98);
  }
}

.enter-right {
  animation: bubble-in-right 300ms cubic-bezier(0, 0, 0.2, 1);
}

.leave-right {
  animation: bubble-out-right 300ms cubic-bezier(0, 0, 0.2, 1) forwards;
}

@keyframes bubble-in-right {
  from {
    opacity: 0;
    transform: translateX(-6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@keyframes bubble-out-right {
  from {
    opacity: 1;
    transform: none;
  }
  to {
    opacity: 0;
    transform: translateX(-4px) scale(0.98);
  }
}

.enter-left {
  animation: bubble-in-left 300ms cubic-bezier(0, 0, 0.2, 1);
}

.leave-left {
  animation: bubble-out-left 300ms cubic-bezier(0, 0, 0.2, 1) forwards;
}

@keyframes bubble-in-left {
  from {
    opacity: 0;
    transform: translateX(6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@keyframes bubble-out-left {
  from {
    opacity: 1;
    transform: none;
  }
  to {
    opacity: 0;
    transform: translateX(4px) scale(0.98);
  }
}

.hover-up:hover {
  transform: translateY(-0.8px);
}

.emotion-slide-enter-active,
.emotion-slide-leave-active {
  transition:
    transform 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94),
    opacity 0.3s ease;
}
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

.dialog-text-lock {
  transition: height 0.25s cubic-bezier(0.25, 0.46, 0.45, 0.94);
}
</style>
