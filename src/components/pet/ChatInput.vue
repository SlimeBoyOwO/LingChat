<template>
  <div
    class="relative z-10 flex w-full justify-center transition-all duration-300 ease-out"
    :class="
      props.visible ? 'translate-y-0 opacity-100' : 'pointer-events-none -translate-y-2 opacity-0'
    "
    :style="{ '--pet-ui-scale': scale }"
  >
    <div
      class="chat-input-container flex items-center rounded-[calc(20px*var(--pet-ui-scale,1))] border border-white/10 bg-neutral-950/50 p-[calc(4px*var(--pet-ui-scale,1))] saturate-200 backdrop-blur-xl"
    >
      <input
        v-model="messageText"
        type="text"
        :placeholder="placeholderText"
        :readonly="!isInputEnabled"
        class="flex-1 border-none bg-transparent p-[calc(5px*var(--pet-ui-scale,1))] text-[calc(13px*var(--pet-ui-scale,1))] text-white placeholder-white/40 outline-none [text-shadow:0_1px_4px_rgba(0,0,0,0.5)]"
        @keyup.enter="send"
        @compositionstart="isCompsing = true"
        @compositionend="isCompsing = false"
      />
      <button
        class="relative flex h-6 items-center gap-1 overflow-hidden rounded-full bg-linear-to-tr from-cyan-500 to-blue-400 px-2 text-sm font-bold text-white shadow-[0_4px_15px_rgba(6,182,212,0.4)] transition-all duration-300 hover:from-cyan-400 hover:to-blue-300 hover:shadow-[0_6px_20px_rgba(6,182,212,0.6)] active:scale-95"
        @click="send"
        :disabled="!isInputEnabled"
      >
        <div
          class="pointer-events-none absolute top-0 left-0 h-1/2 w-full rounded-t-full bg-white/20"
        ></div>
        <Forward />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
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

const isTyping = () => hasDraft();
defineExpose({ isTyping });
</script>

<style scoped></style>
