<template>
  <div
    class="relative z-10 flex w-full items-center justify-center gap-[calc(6px*var(--pet-ui-scale,1))] px-[calc(8px*var(--pet-ui-scale,1))] transition-all duration-300 ease-out"
    :class="
      props.visible ? 'translate-y-0 opacity-100' : 'pointer-events-none -translate-y-2 opacity-0'
    "
    :style="{ '--pet-ui-scale': scale }"
  >
    <div
      class="chat-input-container flex min-w-0 flex-1 items-center rounded-full border border-white/10 bg-neutral-950/50 px-[calc(12px*var(--pet-ui-scale,1))] py-[calc(6px*var(--pet-ui-scale,1))] saturate-200 backdrop-blur-xl"
    >
      <!-- 多行 textarea：Enter 发送 / Shift+Enter 换行，自动增高到 2 行后内部滚动。
           字号只能走内联 style：base.css 里 `input, textarea { font-size: max(16px, 1em) }`
           是无 layer 规则，层叠上压过所有 Tailwind 工具类，会把桌面端字号钉死在 16px，
           于是固有宽度/高度不随 --pet-ui-scale 变化（内联 style 才能盖过它）。
           基准 16px 取该规则当前的实际生效值，保证 scale=1 时外观不变。 -->
      <textarea
        ref="textareaRef"
        v-model="messageText"
        rows="1"
        :placeholder="placeholderText"
        :readonly="!isInputEnabled"
        class="w-full resize-none border-none bg-transparent leading-snug text-white placeholder-white/40 outline-none [text-shadow:0_1px_4px_rgba(0,0,0,0.5)] [&::-webkit-scrollbar]:hidden"
        :style="{ fontSize: 'calc(15px * var(--pet-ui-scale, 1))', maxHeight: '2.75em' }"
        @keydown.enter.exact.prevent="send"
        @compositionstart="isCompsing = true"
        @compositionend="isCompsing = false"
      ></textarea>
    </div>
    <!-- 发送键在输入框外右侧：与输入框垂直居中对齐 -->
    <button
      type="button"
      class="flex size-[calc(32px*var(--pet-ui-scale,1))] shrink-0 items-center justify-center rounded-full border border-white/10 bg-linear-to-tr from-cyan-500 to-blue-400 text-white shadow-[0_4px_15px_rgba(6,182,212,0.4)] transition-all duration-300 hover:from-cyan-400 hover:to-blue-300 active:scale-95 disabled:cursor-not-allowed disabled:opacity-50"
      :disabled="!isInputEnabled"
      @click="send"
    >
      <Forward class="size-[calc(15px*var(--pet-ui-scale,1))]" />
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useUIStore } from "@/stores/modules/ui/ui";
import { useSettingsStore } from "@/stores/modules/settings";
import { useChatInput } from "@/composables/chat/useChatInput";
import { useDialogStatus } from "@/composables/chat/useDialogStatus";
import { useScreenshot } from "@/composables/useScreenshot";
import { Forward } from "lucide-vue-next";

const { t } = useI18n();
const uiStore = useUIStore();
const settingsStore = useSettingsStore();

const { init: initScreenshot, destroy: destroyScreenshot } = useScreenshot();

onMounted(() => {
  initScreenshot();
  grow();
});
onUnmounted(() => {
  destroyScreenshot();
});

const scale = computed(() => settingsStore.pet?.scale || 1.0);

// 状态判定与 input 可用性来自共享实现
const { placeholderState, isInputEnabled } = useDialogStatus();

// 占位符：状态判定共享，文案用桌宠自己的词条
const placeholderText = computed(() => {
  const s = placeholderState.value;
  switch (s.kind) {
    case "hint":
      return s.hint || t("views.pet.chatInput.placeholder");
    case "thinking":
      return s.length > 0
        ? t("views.pet.chatInput.deepThought", { message: s.message, length: s.length })
        : s.message;
    case "waiting":
      return t("views.pet.chatInput.waiting");
    case "responding":
      return t("views.pet.chatInput.chatting");
    case "presenting":
      return "";
    default:
      return t("views.pet.chatInput.placeholderDefault");
  }
});

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
});

// 输入框文本/发送/ASR 接线 —— 与主界面 GameDialog 共用同一实现。
// 别名回原有变量名，模板绑定不变。
const {
  text: messageText,
  isComposing: isCompsing,
  send,
  hasDraft,
} = useChatInput({
  noModelTitleKey: "views.pet.chatInput.noModelTitle",
  noModelMessageKey: "views.pet.chatInput.noModelMessage",
});

const textareaRef = ref<HTMLTextAreaElement | null>(null);

/** 自动增高：先置 auto 再按 scrollHeight；上限由 maxHeight（2 行）兜住，超出内部滚动 */
const grow = () => {
  const el = textareaRef.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${el.scrollHeight}px`;
};

watch([messageText, scale], grow, { flush: "post" });

const isTyping = () => hasDraft();
defineExpose({ isTyping });
</script>

<style scoped></style>
