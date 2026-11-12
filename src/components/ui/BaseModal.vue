<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 scale-100"
      enter-to-class="opacity-100 scale-100"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100 scale-100"
      leave-to-class="opacity-0 scale-100"
    >
      <div v-if="show" class="fixed inset-0 z-9999 flex items-center justify-center p-4">
        <!-- 背景遮罩 -->
        <div class="absolute inset-0 bg-slate-900/60 backdrop-blur-sm" @click="close"></div>

        <!-- 表单弹窗与轻量创建弹窗共用遮罩、标题栏和滚动布局。 -->
        <div
          role="dialog"
          aria-modal="true"
          :aria-labelledby="titleId"
          class="relative z-10 w-full shadow-2xl"
          :class="[
            isGlass ? 'glass-effect text-white' : 'bg-white',
            size === 'lg'
              ? 'flex max-h-[85dvh] max-w-3xl flex-col overflow-hidden rounded-3xl'
              : 'max-w-md rounded-[2.5rem] p-8',
          ]"
          @keydown.esc.stop.prevent="close"
        >
          <!-- 标题栏 -->
          <div
            class="flex shrink-0 items-center justify-between gap-4"
            :class="size === 'lg' ? 'border-b border-white/10 bg-white/5 p-4 sm:p-6' : 'mb-6'"
          >
            <div class="flex min-w-0 items-center gap-4">
              <div
                v-if="$slots.icon"
                class="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-white/10 shadow-inner"
              >
                <slot name="icon" />
              </div>
              <h3
                :id="titleId"
                class="text-xl tracking-tight"
                :class="
                  size === 'lg' && isGlass
                    ? 'font-bold text-white'
                    : isGlass
                      ? 'text-brand font-black'
                      : 'font-black text-slate-800'
                "
              >
                {{ title }}
              </h3>
            </div>
            <button
              type="button"
              :disabled="busy"
              :aria-label="$t('common.close')"
              @click="close"
              class="shrink-0 transition-colors disabled:cursor-not-allowed disabled:opacity-40"
              :class="
                size === 'lg'
                  ? 'flex h-9 w-9 cursor-pointer items-center justify-center rounded-full bg-white/10 text-white hover:bg-white/20'
                  : isGlass
                    ? 'p-1 text-white/60 hover:text-white'
                    : 'p-1 text-slate-400 hover:text-slate-600'
              "
            >
              <span class="text-2xl leading-none font-bold">&times;</span>
            </button>
          </div>

          <!-- 内容区域 -->
          <div
            class="space-y-4"
            :class="size === 'lg' ? 'min-h-0 flex-1 overflow-y-auto p-4 sm:p-6' : ''"
          >
            <slot></slot>
          </div>

          <!-- 底部按钮：默认是「确认创建」，需要自定义动作的调用方用 #footer 覆盖 -->
          <div :class="size === 'lg' ? 'shrink-0 border-t border-white/10 bg-white/5 p-4' : 'mt-8'">
            <slot name="footer">
              <button
                :disabled="busy"
                @click="$emit('confirm')"
                class="w-full rounded-2xl bg-cyan-500 py-4 font-black text-white shadow-lg transition-all hover:bg-cyan-600 active:scale-95"
              >
                {{ $t("ui.baseModal.confirmCreate") }}
              </button>
            </slot>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, useId } from "vue";

const props = withDefaults(
  defineProps<{
    show: boolean;
    title: string;
    /** light = 白色卡片；glass = 深色毛玻璃，与日程面板的卡片/弹窗一致 */
    theme?: "light" | "glass";
    size?: "md" | "lg";
    busy?: boolean;
  }>(),
  { theme: "light", size: "md", busy: false },
);

const isGlass = computed(() => props.theme === "glass");
const titleId = useId();

const emit = defineEmits(["close", "confirm"]);

const close = () => {
  if (!props.busy) emit("close");
};
</script>
