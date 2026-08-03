<template>
  <!--
    背景层必须自己造。窗口是 transparent: true（tauri.conf.json），设置面板之所以
    能透出画面是因为它盖在 MainChat 上；/script-editor 是独立路由，底下什么都没有，
    不给背景就直接透出桌面。Credits.vue 同理显式加了 bg-[#0a0a0c]。
    这里用渐变而不是背景图，避免依赖 Git LFS 资源。
  -->
  <div class="editor-root relative w-full h-full overflow-hidden">
    <div class="bg-layer"></div>

    <EditorHeader
      @playtest="playtest"
      @toggle-shortcut-help="shortcutHelp = true"
      @leave="leave"
    />

    <!-- 主体 -->
    <div class="flex h-[calc(100%-5.5rem)] min-h-0 flex-col">
      <!-- ============ 剧本列表 ============ -->
      <ScriptListPanel
        v-if="!store.detail"
        @new-script="openModal('script')"
      />

      <!-- ============ 章节流程 / 章节编辑 ============ -->
      <FlowTab
        v-else-if="store.tab === 'flow'"
        @new-chapter="openModal('chapter')"
      />

      <!-- ============ 剧本设置 ============ -->
      <ConfigTab v-else-if="store.tab === 'config'" />

      <!-- ============ 角色 ============ -->
      <CharactersTab
        v-else-if="store.tab === 'characters'"
        @new-character="openModal('character')"
        @import-character="openModal('importChar')"
      />

      <!-- ============ 素材 ============ -->
      <AssetsTab v-else-if="store.tab === 'assets'" />

      <!-- ============ AI 助手（Skill Agent） ============ -->
      <div
        v-else-if="store.tab === 'agent-chat'"
        class="flex w-[96%] min-h-0 flex-1 gap-5 mx-auto px-3 py-4"
      >
        <AgentChatPanel />
      </div>

      <MenuPage v-else-if="store.tab === 'agent-settings'">
        <AgentSettingsPanel />
      </MenuPage>

      <!-- ============ 校验 ============ -->
      <ValidateTab v-else />
    </div>

    <!-- 试玩层 -->
    <PreviewStage :from-chapter="previewFrom" />

    <!-- ============ 弹窗 ============ -->
    <EditorModals v-model:modal="modal" />

    <!-- ============ 快捷键表 ============ -->
    <ShortcutHelpPanel
      :visible="shortcutHelp"
      @close="shortcutHelp = false"
    />
  </div>
</template>

<script setup lang="ts">
import { onBeforeRouteLeave, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { MenuPage } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import EditorHeader from '@/components/script-editor/EditorHeader.vue'
import ScriptListPanel from '@/components/script-editor/ScriptListPanel.vue'
import FlowTab from '@/components/script-editor/FlowTab.vue'
import ConfigTab from '@/components/script-editor/ConfigTab.vue'
import CharactersTab from '@/components/script-editor/CharactersTab.vue'
import AssetsTab from '@/components/script-editor/AssetsTab.vue'
import ValidateTab from '@/components/script-editor/ValidateTab.vue'
import EditorModals from '@/components/script-editor/EditorModals.vue'
import ShortcutHelpPanel from '@/components/script-editor/ShortcutHelpPanel.vue'
import AgentChatPanel from '@/components/script-editor/agent/AgentChatPanel.vue'
import AgentSettingsPanel from '@/components/script-editor/agent/AgentSettingsPanel.vue'
import PreviewStage from '@/components/script-editor/PreviewStage.vue'
import { eventQueue } from '@/core/events/event-queue'

const router = useRouter()
const store = useScriptEditorStore()

// ---- 弹窗 ----
const modal = ref<'script' | 'chapter' | 'character' | 'importChar' | null>(null)
const openModal = (which: 'script' | 'chapter' | 'character' | 'importChar') => {
  modal.value = which
}

// ---- 快捷键表 ----
const shortcutHelp = ref(false)

// ---- 其它动作 ----
const previewFrom = ref<string | undefined>(undefined)

const playtest = async () => {
  previewFrom.value = store.level === 'chapter' ? store.chapter?.id : undefined
  await store.startPreview(previewFrom.value)
}

/**
 * 离开编辑器前的统一清理。试玩对自由对话的隔离是「快照 + 还原」：后端
 * PreviewSession 与前端 PreviewStage 各存/还一份状态，这里负责在导航放行前
 * 把两边的还原跑完，并排空事件队列。
 *
 * 关键点：必须 await 完成才放行导航。此前清理放在 onUnmounted（异步、路由不等待），
 * MainChat 会先挂载并 resume 事件队列/读取尚未还原的 line_list，试玩内容就串进
 * 自由对话（历史显示 + AI 上下文）。路由守卫能阻塞导航，从根上消除这个竞态。
 *
 * 幂等：用模块级标志避免与 ✕ leave() / onUnmounted 重复执行。
 */
let exitCleaned = false
const cleanupBeforeExit = async () => {
  if (exitCleaned) return
  exitCleaned = true
  try {
    await store.stopPreview()
  } catch {
    /* 停止试玩失败不阻断离开 */
  }
  // 兜底排空：stopPreview 的清理早于后端任务收尾，IPC 迟到的事件可能在
  // 还原之后才入队（队列已暂停），不排空的话 MainChat 挂载 resume 时会被消费
  eventQueue.clear()
  try {
    await store.flushPendingSave()
  } catch {
    /* 保存失败不阻断离开 */
  }
  // 先落盘再同步，顺序不能反：引擎重扫的是磁盘，没写完就同步等于同步了旧内容
  try {
    await store.syncEngine()
  } catch {
    /* 同步失败不阻断离开 */
  }
}

// 任何离开编辑器的导航（✕、路由跳走、返回手势等）都先完成清理再放行，
// 保证 MainChat 挂载时后端已还原、事件队列干净。
onBeforeRouteLeave(cleanupBeforeExit)

const leave = async () => {
  // 清理统一由路由守卫完成，这里只负责导航
  void router.push('/')
}

// ---- 快捷键 ----
const onKey = (e: KeyboardEvent) => {
  // 在输入框里让位给浏览器原生行为，否则作者想撤销一个词却把整个事件列表
  // 回退了一帧，而且刚敲的字（还没 change 提交）会一起消失。
  const el = e.target as HTMLElement | null
  const typing =
    !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)
  const mod = e.ctrlKey || e.metaKey
  const k = e.key.toLowerCase()

  // Esc 与 ? 不带修饰键，先处理
  if (e.key === 'Escape') {
    if (store.previewing) {
      void store.stopPreview()
    } else if (shortcutHelp.value) {
      shortcutHelp.value = false
    } else if (store.level === 'chapter') {
      store.backToFlow()
    }
    return
  }
  if (!mod && !typing && (e.key === '?' || (e.key === '/' && e.shiftKey))) {
    e.preventDefault()
    shortcutHelp.value = !shortcutHelp.value
    return
  }

  // 试玩期间键盘归游戏，编辑器不抢
  if (store.previewing) return

  if (mod && k === 's') {
    e.preventDefault()
    void store.save()
    return
  }
  if (typing) return

  if (mod) {
    if (k === 'z' && !e.shiftKey) {
      e.preventDefault()
      store.undo()
    } else if ((k === 'z' && e.shiftKey) || k === 'y') {
      e.preventDefault()
      store.redo()
    } else if (k === 'd') {
      e.preventDefault()
      if (store.chapter) store.duplicateEvent(store.selectedEvent)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      void playtest()
    }
    return
  }

  // 以下都只在章节编辑页有意义
  if (store.level !== 'chapter' || !store.chapter) return
  const last = store.chapter.events.length - 1

  if (e.key === 'Delete') {
    e.preventDefault()
    store.removeEvent(store.selectedEvent)
  } else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
    e.preventDefault()
    const step = e.key === 'ArrowUp' ? -1 : 1
    const to = store.selectedEvent + step
    if (to < 0 || to > last) return
    if (e.altKey) store.moveEvent(store.selectedEvent, to)
    else store.selectedEvent = to
  }
}

onMounted(async () => {
  window.addEventListener('keydown', onKey)
  await store.init()
})

onUnmounted(async () => {
  window.removeEventListener('keydown', onKey)
  // 兜底清理：正常情况下路由守卫已 await 完成清理（exitCleaned=true），
  // 这里只在守卫因异常未跑完时补一次，保证试玩停止且游戏会话还原
  await cleanupBeforeExit()
  // 退出编辑器时关闭已打开的剧本——下次从主菜单进入时回到剧本列表
  store.closeScript()
})
</script>

<style scoped>
/* 复杂渐变/伪元素/Vue :deep() 无法用 Tailwind 表达，保留在 scoped 块中 */
.bg-layer {
  position: absolute;
  inset: 0;
  z-index: 0;
  background:
    radial-gradient(900px 500px at 78% 12%, rgba(121, 217, 255, 0.1), transparent 62%),
    radial-gradient(700px 600px at 15% 88%, rgba(90, 140, 190, 0.12), transparent 64%),
    linear-gradient(168deg, #101a26 0%, #16202c 45%, #1b2430 100%);
}
.editor-root > *:not(.bg-layer) {
  position: relative;
  z-index: 1;
}
</style>
