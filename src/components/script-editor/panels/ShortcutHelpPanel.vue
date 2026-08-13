<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/modules/settings'
import {
  DEFAULT_SHORTCUTS,
  SHORTCUT_ACTIONS,
  bindingsEqual,
  captureFromEvent,
  formatBinding,
  type ShortcutAction,
  type ShortcutBinding,
} from '@/utils/shortcuts'

const { t } = useI18n()
const props = defineProps<{ visible: boolean }>()
const emit = defineEmits<{ close: [] }>()

const settings = useSettingsStore()

/** 动作 → 描述词条 key（与 shortcutHelp 段一致） */
const DESC_KEYS: Record<ShortcutAction, string> = {
  save: 'scriptEditor.shortcutHelp.save',
  undo: 'scriptEditor.shortcutHelp.undo',
  redo: 'scriptEditor.shortcutHelp.redo',
  copyEvent: 'scriptEditor.shortcutHelp.copyEvent',
  playtest: 'scriptEditor.shortcutHelp.playtest',
  deleteEvent: 'scriptEditor.shortcutHelp.deleteEvent',
  moveCursor: 'scriptEditor.shortcutHelp.moveCursor',
  moveEvent: 'scriptEditor.shortcutHelp.moveEvent',
  esc: 'scriptEditor.shortcutHelp.esc',
  shortcutHelp: 'scriptEditor.shortcutHelp.openTable',
  expandProps: 'scriptEditor.shortcutHelp.expandProps',
}

// ---- 捕获模式：点击某行后监听下一次 keydown 作为新键位 ----
const recording = ref<ShortcutAction | null>(null)
const conflictName = ref<string>('')
const singleKeyBlocked = ref(false)

const onCaptureKey = (e: KeyboardEvent) => {
  if (!recording.value) return
  e.preventDefault()
  e.stopPropagation()
  const result = captureFromEvent(e)
  // 纯修饰键：组合键进行中，继续等待（不取消、不绑定）
  if (result.kind === 'ignore') return
  // Esc 取消捕获
  if (result.kind === 'cancel') {
    recording.value = null
    conflictName.value = ''
    singleKeyBlocked.value = false
    return
  }
  // 无修饰的普通字符键：拒绝并提示，保持捕获让用户重新按
  if (result.kind === 'blocked') {
    singleKeyBlocked.value = true
    return
  }
  const binding = result.binding
  singleKeyBlocked.value = false
  // 冲突检测：其他动作已占用同一组合则拒绝
  const clash = SHORTCUT_ACTIONS.find(
    (a) => a !== recording.value && bindingsEqual(settings.shortcuts[a], binding),
  )
  if (clash) {
    conflictName.value = t(DESC_KEYS[clash])
    return
  }
  settings.update(`shortcuts.${recording.value}`, binding)
  recording.value = null
  conflictName.value = ''
}

watch(
  () => props.visible,
  (v) => {
    // 面板关闭时清掉捕获态，避免残留监听
    if (!v) {
      recording.value = null
      conflictName.value = ''
      singleKeyBlocked.value = false
    }
  },
)

onUnmounted(() => {
  recording.value = null
})

// 面板打开且处于捕获模式时挂全局监听
watch(recording, (v) => {
  if (v) window.addEventListener('keydown', onCaptureKey, true)
  else window.removeEventListener('keydown', onCaptureKey, true)
})

const restoreDefaults = () => {
  settings.update('shortcuts', { ...DEFAULT_SHORTCUTS })
}

const rowBinding = (a: ShortcutAction) => settings.shortcuts[a] as ShortcutBinding
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
          class="w-[min(460px,92vw)]
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

          <p class="mb-3
            text-[0.72rem]
            leading-[1.7]
            text-white/40">
            {{ t('scriptEditor.shortcutHelp.customizeHint') }}
          </p>

          <div
            v-for="a in SHORTCUT_ACTIONS"
            :key="a"
            class="flex
              items-center
              gap-3
              py-1.5
              text-[0.78rem]
              leading-[1.8]
              text-white/70
              border-t
              border-white/[0.06]
              [&:first-child]:border-t-0"
            :class="
              recording === a
                ? 'text-brand'
                : `cursor-pointer
                  hover:text-white`
            "
            @click="recording = a"
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
            >
              {{
                recording === a
                  ? t('scriptEditor.shortcutHelp.recording')
                  : formatBinding(rowBinding(a))
              }}
            </kbd>
            <span class="min-w-0
              flex-1">{{ t(DESC_KEYS[a]) }}</span>
            <span
              v-if="recording === a && conflictName"
              class="shrink-0
                text-[0.68rem]
                text-yellow-300"
            >
              {{ t('scriptEditor.shortcutHelp.conflict', { name: conflictName }) }}
            </span>
            <span
              v-else-if="recording === a && singleKeyBlocked"
              class="shrink-0
                text-[0.68rem]
                text-yellow-300"
            >
              {{ t('scriptEditor.shortcutHelp.singleKeyBlocked') }}
            </span>
          </div>

          <div class="flex
            justify-end
            mt-4">
            <button
              class="inline-flex
                items-center
                gap-1
                border
                border-white/10
                rounded-lg
                px-3
                py-[0.3rem]
                text-[0.78rem]
                whitespace-nowrap
                text-white/70
                bg-white/6
                transition-all
                duration-200
                hover:enabled:text-white
                hover:enabled:bg-white/[0.12]"
              @click="restoreDefaults"
            >
              {{ t('scriptEditor.shortcutHelp.restoreDefault') }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
