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

        <!-- 模态框主体 -->
        <div
          class="relative z-10 w-full max-w-md rounded-[2.5rem] p-8 shadow-2xl"
          :class="isGlass ? 'glass-effect' : 'bg-white'"
        >
          <!-- 标题栏 -->
          <div class="mb-6 flex items-center justify-between">
            <h3
              class="text-xl font-black tracking-tight"
              :class="isGlass ? 'text-brand' : 'text-slate-800'"
            >
              {{ title }}
            </h3>
            <button
              @click="close"
              class="p-1 transition-colors"
              :class="
                isGlass ? 'text-white/60 hover:text-white' : 'text-slate-400 hover:text-slate-600'
              "
            >
              <span class="text-2xl leading-none font-bold">&times;</span>
            </button>
          </div>

          <!-- 内容区域 -->
          <div class="space-y-4">
            <slot></slot>
          </div>

          <!-- 底部按钮：默认是「确认创建」，需要自定义动作的调用方用 #footer 覆盖 -->
          <div class="mt-8">
            <slot name="footer">
              <button
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
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    show: boolean;
    title: string;
    /** light = 白色卡片；glass = 深色毛玻璃，与日程面板的卡片/弹窗一致 */
    theme?: "light" | "glass";
  }>(),
  { theme: "light" },
);

const isGlass = computed(() => props.theme === "glass");

const emit = defineEmits(["close", "confirm"]);

const close = () => {
  emit("close");
};
</script>
