<template>
  <!--
    背景层必须自己造。窗口是 transparent: true（tauri.conf.json），设置面板之所以
    能透出画面是因为它盖在 MainChat 上；/script-editor 是独立路由，底下什么都没有，
    不给背景就直接透出桌面。Credits.vue 同理显式加了 bg-[#0a0a0c]。
    这里用渐变而不是背景图，避免依赖 Git LFS 资源。
  -->
  <div class="editor-root
    relative
    w-full
    h-full
    overflow-hidden">
    <div class="bg-layer"></div>

    <EditorHeader
      @playtest="playtest"
      @toggle-shortcut-help="shortcutHelp = true"
      @leave="leave"
    />

    <!-- 试玩前置条件不满足时的常驻提示。等作者点了「试玩」才报，他会先对着
         一个卡住不动的画面困惑一阵 —— 那正是这条横幅要省掉的时间。 -->
    <div
      v-if="store.detail && store.readiness && !store.readiness.ok"
      class="flex
        items-center
        gap-2.5
        mx-5
        mb-2
        border
        border-amber-300/30
        rounded-lg
        px-3
        py-[7px]
        text-[0.76rem]
        leading-[1.7]
        text-amber-100/90
        bg-amber-300/10"
    >
      <span
        class="shrink-0
          border
          border-amber-300/40
          rounded-full
          px-2
          py-px
          text-[0.66rem]
          font-semibold
          text-amber-300"
        >试玩会卡住</span
      >
      <span>{{ store.readiness.reason }}</span>
    </div>

    <!-- 主体 -->
    <div class="flex
      h-[calc(100%-5.5rem)]
      min-h-0
      flex-col">
      <!-- ============ 剧本列表 ============ -->
      <ScriptListPanel
        v-if="!store.detail && store.tab === 'flow'"
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
        class="flex
          w-[96%]
          min-h-0
          flex-1
          gap-5
          mx-auto
          px-3
          py-4"
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
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
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
import { AssetKind, AssetScope, AssetFile, Diagnostic } from '@/api/services/script-editor'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open as openDialog } from '@tauri-apps/plugin-dialog'

const router = useRouter()
const store = useScriptEditorStore()

type TabKey =
  | 'flow'
  | 'config'
  | 'characters'
  | 'assets'
  | 'validate'
  | 'agent-chat'
  | 'agent-settings'

const tabs: {
  key: TabKey
  label: string
  icon: 'adventure' | 'setting' | 'character' | 'background' | 'achievement' | 'bot' | 'sliders'
}[] = [
  { key: 'flow', label: '章节流程', icon: 'adventure' },
  { key: 'config', label: '剧本设置', icon: 'setting' },
  { key: 'characters', label: '角色', icon: 'character' },
  { key: 'assets', label: '素材', icon: 'background' },
  { key: 'validate', label: '校验', icon: 'achievement' },
  { key: 'agent-chat', label: '剧本导师', icon: 'bot' },
  { key: 'agent-settings', label: '导师设置', icon: 'sliders' },
]

const assetKinds: { key: AssetKind; label: string }[] = [
  { key: 'background', label: '背景图' },
  { key: 'pic', label: '插图' },
  { key: 'music', label: '背景音乐' },
  { key: 'sound', label: '音效' },
  { key: 'ambient', label: '环境音' },
]

// ---- 素材页 ----

const isImageKind = (k: AssetKind) => k === 'background' || k === 'pic'

/** 绝对路径 → webview 能加载的 asset URL，与 GameBackground / GameRoleAvatar 同一套 */
const assetUrl = (path: string) => convertFileSrc(path)

const filesOf = (scope: AssetScope, kind: AssetKind): AssetFile[] =>
  store.assetFiles[scope]?.[kind] ?? []

// 音效没有全局目录（issue #6），只展示「本剧本」一列；其余素材仍是「本剧本 + 全局」
const scopesFor = (kind: AssetKind): AssetScope[] =>
  kind === 'sound' ? ['script'] : ['script', 'global']

const humanSize = (n: number) => {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

// ---- 素材音频播放速度 ----
const audioEls: Record<string, HTMLAudioElement | null> = {}
const audioRates = reactive<Record<string, number>>({})
const speedMenu = ref<string | null>(null)
const setAudioRef = (path: string) => (el: unknown) => {
  audioEls[path] = el as HTMLAudioElement | null
  if (!(path in audioRates)) audioRates[path] = 1
}
const onDocClick = () => {
  speedMenu.value = null
}
const setRate = (path: string, rate: number) => {
  const a = audioEls[path]
  if (a) {
    a.playbackRate = rate
    audioRates[path] = rate
  }
}

// ---- nav 指示条（与 SettingsNav 同一套做法）----
const navEl = ref<HTMLElement | null>(null)
const indicatorEl = ref<HTMLElement | null>(null)
const tabRefs: Record<string, HTMLElement | null> = {}

const setTabRef = (key: string, el: unknown) => {
  tabRefs[key] = (el as { $el?: HTMLElement } | null)?.$el ?? null
}

/**
 * 指示条定位。
 *
 * 之前它经常停在空白处，原因是位置只在「切标签」和「换剧本」时算一次，而
 * 按钮宽度会在别的时刻变：校验跑完后「校验」上多一个错误数角标、窗口跨过
 * xl 断点时文字标签整体显隐、字体加载完成后每个字的宽度都变。nav 是
 * `justify-content: center`，任何一个按钮变宽都会把**所有**按钮推走，于是
 * 上一次算出来的 left 就落到了两个按钮中间的空当里。
 *
 * 所以这里不再指望「在正确的时刻算一次」，而是让尺寸变化自己来触发重算：
 * ResizeObserver 同时盯着 nav 和每一个按钮。另外用 getBoundingClientRect
 * 相对 nav 求差而不是 offsetLeft —— 后者依赖 offsetParent 恰好是 nav，
 * 一旦有人给中间层加了 position 就会静默偏移。
 */
const moveIndicator = async (animate = true) => {
  await nextTick()
  const bar = indicatorEl.value
  const nav = navEl.value
  if (!bar || !nav) return
  const target = tabRefs[store.tab]
  bar.style.transition = animate
    ? 'left 0.3s cubic-bezier(0.18, 0.89, 0.32, 1), width 0.3s cubic-bezier(0.18, 0.89, 0.32, 1)'
    : 'none'
  if (!target) {
    // 目标不在了就收起来。早先这里是直接 return，于是指示条保持在上一次的
    // 位置不动 —— 那正是「出现在空白处」最刺眼的一种。
    bar.style.width = '0px'
    return
  }
  const navBox = nav.getBoundingClientRect()
  const box = target.getBoundingClientRect()
  bar.style.left = `${box.left - navBox.left + nav.scrollLeft}px`
  bar.style.width = `${box.width}px`
}

watch(
  () => store.tab,
  () => moveIndicator(),
)
watch(
  () => store.detail?.package.key,
  () => moveIndicator(),
)

let navObserver: ResizeObserver | null = null

const observeNav = () => {
  if (typeof ResizeObserver === 'undefined' || !navEl.value) return
  // 不加动画：这类重算是「布局变了跟着修正」，滑一下反而像在乱动
  navObserver = new ResizeObserver(() => void moveIndicator(false))
  navObserver.observe(navEl.value)
  for (const el of Object.values(tabRefs)) if (el) navObserver.observe(el)
}

const switchTab = (key: TabKey) => {
  if (!store.detail && key !== 'flow' && key !== 'agent-chat' && key !== 'agent-settings') return
  store.tab = key
  if (key === 'validate') void store.runValidation()
  if (key === 'assets') {
    void store.refreshGlobalAssets()
    void store.refreshAssetFiles()
  }
  if (key === 'characters') void store.refreshGlobalCharacters()
  if (key === 'flow') {
    // 有剧本：回到流程图时强制走一遍「落盘 → 重新校验」，图上画的才是磁盘里的真状态
    if (store.detail && store.level === 'flow') void store.backToFlow()
    // 无剧本：AI 助手可能刚写好新剧本，回来时刷新列表让它出现
    else if (!store.detail) void store.refreshScripts()
  }
}

// ---- 面包屑 ----
/** 没打开剧本时，顶部面包屑按当前 tab 显示所在区块（AI 助手无剧本也能进） */
const noDetailTitle = computed(() => {
  switch (store.tab) {
    case 'agent-chat':
      return '剧本导师'
    case 'agent-settings':
      return '导师设置'
    default:
      return '剧本列表'
  }
})

const saveLabel = computed(() => {
  if (store.saving) return '正在保存…'
  if (store.dirty) return '有未保存改动'
  if (store.lastSavedAt) {
    const d = new Date(store.lastSavedAt)
    return `已自动保存 · ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(
      2,
      '0',
    )}`
  }
  return '已保存'
})

// ---- 校验页 ----
const diagnosticsOf = (chapterId: string) =>
  (store.report?.diagnostics ?? []).filter((d) => d.chapter === chapterId)

const chapterHas = (chapterId: string) => diagnosticsOf(chapterId).length > 0

const jumpTo = async (d: Diagnostic) => {
  if (!d.chapter) {
    store.tab = 'config'
    return
  }
  store.tab = 'flow'
  if (store.chapter?.id !== d.chapter) {
    // openChapter 可能失败（读盘出错），失败时不要把 selectedEvent 设成别的章节的下标
    if (!(await store.openChapter(d.chapter))) return
  } else {
    store.level = 'chapter'
  }
  if (d.eventIndex !== undefined) store.selectedEvent = d.eventIndex
}

// ---- 剧本设置 ----
const configDraft = reactive<Record<string, unknown>>({})

watch(
  () => store.detail?.storyConfig,
  (cfg) => {
    for (const k of Object.keys(configDraft)) delete configDraft[k]
    Object.assign(configDraft, JSON.parse(JSON.stringify(cfg ?? {})))
  },
  { immediate: true, deep: false },
)

const setConfig = (key: string, value: unknown) => {
  configDraft[key] = value
}

const adventureObj = computed<Record<string, unknown>>(() => {
  const a = configDraft.adventure
  return a && typeof a === 'object' ? (a as Record<string, unknown>) : {}
})

const isAdventure = computed(() => adventureObj.value.is_adventure === true)

const adventureField = (k: string) => {
  const v = adventureObj.value[k]
  return v === undefined || v === null ? '' : String(v)
}

const setAdventure = (k: string, v: unknown) => {
  const next = { ...adventureObj.value, [k]: v }
  configDraft.adventure = next
}

/** 抽出来是因为内联写法要带 `(e.target as HTMLInputElement)`，模板里读起来太吵 */
const onAdventureText = (k: string, e: Event) =>
  setAdventure(k, (e.target as HTMLInputElement).value)

const onAdventureNumber = (k: string, e: Event) =>
  setAdventure(k, Number((e.target as HTMLInputElement).value) || 0)

const toggleAdventure = (on: boolean) => {
  if (on) {
    setAdventure('is_adventure', true)
  } else {
    // 关掉只改标志，其余字段原样留着 —— 作者可能只是临时关掉
    setAdventure('is_adventure', false)
  }
}

const saveConfig = () => {
  void store.saveStoryConfig(JSON.parse(JSON.stringify(configDraft)))
}

// ---- 素材导入 ----
const IMAGE_EXT = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif']
const AUDIO_EXT = ['mp3', 'wav', 'ogg', 'flac', 'm4a']

const importAsset = async (kind: AssetKind, scope: AssetScope) => {
  const isImage = kind === 'background' || kind === 'pic'
  const picked = await openDialog({
    multiple: false,
    filters: [{ name: isImage ? '图片' : '音频', extensions: isImage ? IMAGE_EXT : AUDIO_EXT }],
  })
  if (typeof picked !== 'string') return
  await store.uploadAsset(kind, scope, picked)
}

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
