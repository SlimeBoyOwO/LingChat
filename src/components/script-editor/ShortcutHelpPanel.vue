<script setup lang="ts">
defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const SHORTCUTS: { keys: string; desc: string }[] = [
  { keys: 'Ctrl / ⌘ + S', desc: '立刻保存（平时是改完自动存，这条是给不放心的人的）' },
  { keys: 'Ctrl / ⌘ + Z', desc: '撤销' },
  { keys: 'Ctrl / ⌘ + Shift + Z', desc: '恢复刚才撤销的操作（Ctrl+Y 也行）' },
  { keys: 'Ctrl / ⌘ + D', desc: '复制选中的事件' },
  { keys: 'Ctrl / ⌘ + Enter', desc: '从当前位置试玩' },
  { keys: 'Delete', desc: '删除选中的事件' },
  { keys: '↑ / ↓', desc: '在事件之间移动光标' },
  { keys: 'Alt + ↑ / ↓', desc: '把选中的事件上移 / 下移' },
  { keys: 'Esc', desc: '结束试玩 / 返回上一层' },
  { keys: '?', desc: '打开这张表' },
]
</script>

<template>
  <Teleport to="body">
    <Transition
      enter-active-class="transition-opacity duration-200 ease"
      leave-active-class="transition-opacity duration-200 ease"
      enter-from-class="opacity-0"
      leave-to-class="opacity-0"
    >
      <div
        v-if="visible"
        class="modal-mask fixed inset-0 z-[9999] flex items-center justify-center p-4 backdrop-blur-md bg-black/55"
        @click.self="emit('close')"
      >
        <div class="w-[min(440px,92vw)] max-h-[86vh] overflow-y-auto border border-white/12.5 rounded-xl py-4 px-[18px] pb-[18px] bg-[rgba(12,20,30,0.86)] backdrop-blur-lg backdrop-saturate-[1.4] shadow-[0_8px_32px_rgba(0,0,0,0.45),inset_0_1px_1px_rgba(255,255,255,0.06)]">
          <div class="flex items-center gap-2 border-b-2 border-brand pb-2 mb-4">
            <h4 class="font-semibold text-white">快捷键</h4>
            <button
              class="ml-auto text-white/50 transition-all duration-300 hover:text-brand hover:rotate-90"
              @click="emit('close')"
            >
              ✕
            </button>
          </div>
          <div
            v-for="s in SHORTCUTS"
            :key="s.keys"
            class="flex items-baseline gap-3 py-1.5 text-[0.78rem] leading-[1.8] text-white/70 border-t border-white/[0.06] [&:first-child]:border-t-0"
          >
            <kbd class="shrink-0 min-w-[148px] border border-white/[0.14] rounded-[5px] px-2 py-0.5 font-mono text-[0.7rem] text-brand bg-white/5">{{ s.keys }}</kbd>
            <span>{{ s.desc }}</span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
