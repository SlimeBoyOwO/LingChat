<script setup lang="ts">
import { useI18n } from 'vue-i18n'

const { t } = useI18n()
defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const SHORTCUTS: { keys: string; desc: string }[] = [
  { keys: 'Ctrl / ⌘ + S', desc: 'scriptEditor.shortcutHelp.save' },
  { keys: 'Ctrl / ⌘ + Z', desc: 'scriptEditor.shortcutHelp.undo' },
  { keys: 'Ctrl / ⌘ + Shift + Z', desc: 'scriptEditor.shortcutHelp.redo' },
  { keys: 'Ctrl / ⌘ + D', desc: 'scriptEditor.shortcutHelp.copyEvent' },
  { keys: 'Ctrl / ⌘ + Enter', desc: 'scriptEditor.shortcutHelp.playtest' },
  { keys: 'Delete', desc: 'scriptEditor.shortcutHelp.deleteEvent' },
  { keys: '↑ / ↓', desc: 'scriptEditor.shortcutHelp.moveCursor' },
  { keys: 'Alt + ↑ / ↓', desc: 'scriptEditor.shortcutHelp.moveEvent' },
  { keys: 'Esc', desc: 'scriptEditor.shortcutHelp.esc' },
  { keys: '?', desc: 'scriptEditor.shortcutHelp.openTable' },
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
        class="modal-mask
          fixed
          inset-0
          z-[9999]
          flex
          items-center
          justify-center
          p-4
          backdrop-blur-md
          bg-black/55"
        @click.self="emit('close')"
      >
        <div
          class="w-[min(440px,92vw)]
            max-h-[86vh]
            overflow-y-auto
            border
            border-white/12.5
            rounded-xl
            py-4
            px-[18px]
            pb-[18px]
            bg-[rgba(12,20,30,0.86)]
            backdrop-blur-lg
            backdrop-saturate-[1.4]
            shadow-[0_8px_32px_rgba(0,0,0,0.45),inset_0_1px_1px_rgba(255,255,255,0.06)]"
        >
          <div class="flex
            items-center
            gap-2
            border-b-2
            border-brand
            pb-2
            mb-4">
            <h4 class="font-semibold
              text-white">{{ t('scriptEditor.shortcutHelp.title') }}</h4>
            <button
              class="ml-auto
                text-white/50
                transition-all
                duration-300
                hover:text-brand
                hover:rotate-90"
              @click="emit('close')"
            >
              ✕
            </button>
          </div>
          <div
            v-for="s in SHORTCUTS"
            :key="s.keys"
            class="flex
              items-baseline
              gap-3
              py-1.5
              text-[0.78rem]
              leading-[1.8]
              text-white/70
              border-t
              border-white/[0.06]
              [&:first-child]:border-t-0"
          >
            <kbd
              class="shrink-0
                min-w-[148px]
                border
                border-white/[0.14]
                rounded-[5px]
                px-2
                py-0.5
                font-mono
                text-[0.7rem]
                text-brand
                bg-white/5"
              >{{ s.keys }}</kbd
            >
            <span>{{ t(s.desc) }}</span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
