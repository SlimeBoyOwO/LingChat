<template>
  <!--
    外层**常驻**,不用 v-if/v-show：TypeWriter 在构造时缓存 element，节点一旦被销毁
    重建，后续字符就会写进已脱离文档的旧节点 —— 表现为「气泡在、文字空白」。
    显隐与动效交给 CSS 类 + 作用域关键帧（见文件末尾 .enter/.leave）。
  -->
  <div
    class="absolute inset-x-0 bottom-(--tail) z-30 flex cursor-pointer items-end justify-center px-2"
    :class="visible ? 'enter' : 'leave pointer-events-none'"
    @click="emit('advance')"
  >
    <div
      ref="bubbleRef"
      class="hover-up relative w-[85%] rounded-[calc(20px*var(--pet-ui-scale,1))] border border-white/10 bg-neutral-950/50 px-[calc(18px*var(--pet-ui-scale,1))] py-[calc(6px*var(--pet-ui-scale,1))] text-white backdrop-blur-xl backdrop-saturate-200 transition-all duration-300 [text-shadow:0_1px_4px_rgba(0,0,0,0.5)] hover:scale-[1.02] hover:border-white/20 hover:bg-neutral-950/65"
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

      <!-- 正文：高度按文本实测动态设定（见 applyTextHeight），上限之外在盒内滚动 -->
      <div
        ref="textRef"
        class="dialog-text-lock [scrollbar-width:none] overflow-y-auto pb-[0.4em] text-[calc(15px*var(--pet-ui-scale,1))] leading-snug font-medium break-all whitespace-pre-line [text-shadow:0_0_3px_rgba(0,0,0,0.9),0_1px_4px_rgba(0,0,0,0.5)] [&::-webkit-scrollbar]:hidden"
      ></div>

      <!-- 长尾：位于气泡底边下方，落在容器抬升出来的预留区里（原版用 -bottom-2.5/-2，
           但那会伸到窗口底边之外被裁；容器已按 TAIL_OVERHANG 抬升，这里等价落位） -->
      <div
        class="absolute -bottom-2.5 left-1/2 h-0 w-0 -translate-x-1/2 border-r-10 border-l-10 border-t-white/10 border-r-transparent border-l-transparent drop-shadow-md"
      ></div>
      <div
        class="absolute -bottom-2 left-1/2 h-0 w-0 -translate-x-1/2 border-t-8 border-r-8 border-l-8 border-t-white/8 border-r-transparent border-l-transparent"
      ></div>

      <!-- 通知：钉在气泡顶边上方。气泡高度随文本变化，通知因此始终紧贴气泡顶，
           而不是钉在窗口顶（那样短气泡与通知之间会隔开一大片空白） -->
      <div
        v-if="uiStore.notification.isVisible"
        class="absolute inset-x-0 bottom-full mb-1 flex justify-center"
        :style="{ maxHeight: 'var(--notify-h)' }"
      >
        <PetNotification />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * 气泡（桌宠显示层）。**受控组件**：可见性与文本都由父级给，自己不读对话状态、
 * 不碰事件队列 —— 气泡窗与宠物窗是两个 webview，各自的事件队列互不相识，
 * 让气泡自己跑状态机就会与镜像来的状态打架，气泡永远停在隐藏态。
 *
 * 只有打字机动画归本组件，因为它绑在 DOM 上。
 */
import { ref, watch } from "vue";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useTypeWriter } from "@/composables/ui/useTypeWriter";
import { createCharRevealWriter } from "@/utils/typewriter/charReveal";
import { charRevealCharHtml } from "@/utils/typewriter/charHtml";
import PetNotification from "./PetNotification.vue";

const props = defineProps<{
  visible: boolean;
  /** 完整台词；与上一次不同才（重新）打字 */
  line: string;
  emotion?: string;
  speed?: number;
  /** 直接整段显示、不播打字动画 */
  instant?: boolean;
  /** 气泡高度上限（px），父级按带高算好传入；正文超出部分在盒内滚动 */
  maxHeight: number;
}>();

const emit = defineEmits<{ advance: []; drained: [] }>();

const uiStore = useUIStore();

const textRef = ref<HTMLElement | null>(null);
const bubbleRef = ref<HTMLElement | null>(null);

// 逐字符淡入+上浮渲染器（颜色/阴影继承气泡样式）
const charReveal = createCharRevealWriter({ charHtml: charRevealCharHtml });

const { startTyping, stopTyping, finishTyping, isTyping } = useTypeWriter(
  textRef,
  undefined,
  // 正文是普通 <div>（非 textarea/input），必须提供 writeFn 做增量字符渲染
  charReveal.writeFn,
);

/**
 * 去重与"重播"判据。
 *
 * 背景：本组件会在三种情况下被重渲染 —— 切情绪、改尺寸、以及隐藏后重新显示。
 * 前两者显示区内容还在，不该动；后者内容已随 stopTyping 清空，必须重画，
 * 但**不能重播打字机**（用户要求打字机只在新台词时播）。
 *
 * 关键在于"内容还在不在"必须与"是不是新台词"分开判断：
 * 之前把两者揉进一个条件里，导致新台词也被当成"重画"，打字机再没跑过。
 */
const displayEmpty = ref(true);
/** 已完整呈现的台词（渲染收尾时记录）；null = 显示区没有有效内容 */
const shownLine = ref<string | null>(null);

/**
 * 渲染代次：每次渲染自增，旧渲染 await 回来时若代次已变就自我作废，
 * 避免旧渲染的收尾踩到新渲染、或误发 drained 打乱调度。
 */
let renderToken = 0;

/**
 * 设定正文高度：按整段文本实测，钳到高度上限。
 *
 * 必须**先锁到最终高度再开始打字** —— 否则盒子会随逐字换行而抖动。
 * 高度变化由 `.dialog-text-lock` 的 height 过渡负责，所以气泡是"长"出来的。
 * 上限之外的部分留给 `overflow-y` 滚动，因此盒子永远不会超出窗口。
 */
const applyTextHeight = (line: string) => {
  const el = textRef.value;
  if (!el) return;
  if (!line.trim()) {
    el.style.height = "0px";
    return;
  }
  // 离屏克隆量"整段渲染后"的高度：借用真实元素的行宽与全部排版样式
  const clone = el.cloneNode(false) as HTMLDivElement;
  clone.style.cssText = `position:fixed;left:-9999px;top:0;visibility:hidden;height:auto;overflow:visible;width:${el.clientWidth}px`;
  el.parentElement?.appendChild(clone);
  charReveal.renderInstant(clone, line);
  const measured = clone.offsetHeight;
  clone.remove();
  charReveal.reset();
  el.style.height = `${Math.min(measured, props.maxHeight)}px`;
};

/** 清空显示区并把"已呈现"作废 —— 内容没了，就不能再说这一句画过了 */
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

/**
 * 重画这一句。
 * @param instant 整段显示、不播打字机
 */
const render = async (line: string, instant: boolean) => {
  const token = ++renderToken;

  stopTyping();
  if (textRef.value) {
    textRef.value.innerHTML = "";
    // 归零并强制重排，让浏览器把"0"记为过渡起点，
    // 随后设到新高度 → .dialog-text-lock 的 height 过渡才会生效
    textRef.value.style.height = "0px";
    void textRef.value.offsetHeight;
  }
  charReveal.reset();
  applyTextHeight(line);

  if (instant) {
    if (textRef.value) charReveal.renderInstant(textRef.value, line);
    displayEmpty.value = false;
    shownLine.value = line;
    emit("drained");
    return;
  }

  await startTyping(line, props.speed);
  // 已被更新的渲染接管（或被隐藏）：不记已呈现、也不发 drained，它属于上一句
  if (token !== renderToken) return;
  displayEmpty.value = false;
  shownLine.value = line;
  if (!isTyping.value) emit("drained");
};

watch(
  () => [props.visible, props.line, props.instant] as const,
  ([visible, line, instant], prev) => {
    if (!visible || !line) {
      // 作废在跑的渲染，避免它稍后发 drained 干扰下一句的调度
      renderToken++;
      clearDisplay();
      return;
    }

    const sameLine = shownLine.value === line;
    const stillShown = prev?.[0] === true && !displayEmpty.value;

    // 状态没变（切情绪、改尺寸、通知变化）→ 什么都不做，保持气泡现状
    if (stillShown && sameLine) return;

    // 显示区已空但显示状态没变（隐藏后重新显示）→ 重画，但整段复现，不重播打字机
    const restoring = prev?.[0] === false && sameLine;
    void render(line, instant || restoring);
  },
  { immediate: true, flush: "post" },
);

defineExpose({
  isTyping,
  /** 补全当前打字动画（点头像时先补全、不推进） */
  finishTyping,
  bubbleRef,
});
</script>

<style scoped>
/**
 * 出现/消失动效 —— 复刻自 5f61eeec「feat: 添加气泡/通知换位动效」与当时 DialogueBox 的定义：
 *   容器 transition-all duration-300 ease-out，位移 ±2（translate-y-0 ↔ -translate-y-2）配合 opacity 0 ↔ 100。
 *
 * 两处改动：
 *   1. 用关键帧而非类切换：本组件节点常驻（见模板注释），必须在没有"上一次状态"的
 *      情况下也能播出进场动效，animation 天然满足。
 *   2. 位移方向改为进场自下而上、退场向下（原版两个方向都朝上，进场像缩回宠物头顶）。
 */
.enter {
  animation: bubble-in 300ms cubic-bezier(0, 0, 0.2, 1);
}

.leave {
  animation: bubble-out 300ms cubic-bezier(0, 0, 0.2, 1) forwards;
}

@keyframes bubble-in {
  from {
    opacity: 0;
    transform: translateY(6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@keyframes bubble-out {
  from {
    opacity: 1;
    transform: none;
  }
  to {
    opacity: 0;
    transform: translateY(4px) scale(0.98);
  }
}

/* 悬浮上浮 0.8px。原版写的是 hover:-translate-y-0.2，但 Tailwind 不为 .2 这种
   非 scale 小数算子生成规则，该效果实际从未生效；这里用 scoped 规则补上。 */
.hover-up:hover {
  transform: translateY(-0.8px);
}

/* 情绪标签切换：上一个向左滑出，下一个从右侧滑入（推挤效果） */
.emotion-slide-enter-active,
.emotion-slide-leave-active {
  transition:
    transform 0.3s cubic-bezier(0.25, 0.46, 0.45, 0.94),
    opacity 0.3s ease;
}
/* 离开中的旧情绪脱离文档流，覆盖在新情绪上方向左滑出，容器宽度由新情绪决定 */
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

/* 打字期间高度锁定为最终高度；行切换时高度平滑扩展/收缩 */
.dialog-text-lock {
  transition: height 0.25s cubic-bezier(0.25, 0.46, 0.45, 0.94);
}
</style>
