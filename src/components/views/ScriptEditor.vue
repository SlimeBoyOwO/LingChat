<template>
  <!--
    背景层必须自己造。窗口是 transparent: true（tauri.conf.json），设置面板之所以
    能透出画面是因为它盖在 MainChat 上；/script-editor 是独立路由，底下什么都没有，
    不给背景就直接透出桌面。Credits.vue 同理显式加了 bg-[#0a0a0c]。

    结构：bg-layer（渐变兜底）→ bg-image（背景图，模糊+压暗）→ bg-dim-mask（可调遮罩）。
    默认背景图是主菜单同款的 background2.png —— 它是 Git LFS 资产，无 LFS 的
    开发环境读出来是 131 字节指针，加载失败时浏览器自动跳过该图层露出渐变，
    因此本地开发看到兜底渐变、CI 构建产物看到真实图，两种环境都不会破图。
  -->
  <div class="editor-root
    relative
    w-full
    h-full
    overflow-hidden">
    <div
      class="bg-layer"
      :style="{
        '--bg-blur': `${store.editorBg.blur}px`,
        '--bg-dim': String(store.editorBg.dim),
      }"
    >
      <div
        class="bg-image"
        :style="{ backgroundImage: `url(${bgImageSrc})` }"
      ></div>
      <div class="bg-dim-mask"></div>
    </div>

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
        >{{ t('scriptEditor.scriptEditor.playtestBlocked') }}</span
      >
      <span>{{ store.readiness.reason }}</span>
    </div>

    <!-- 主体：Tab 切换复用设置面板的推入推出过渡，方向随 Tab 顺序前进/后退 -->
    <div class="relative
      h-[calc(100%-5.5rem)]
      min-h-0">
      <Transition :name="transitionName">
        <component
          :is="currentTabComponent"
          :key="tabKey"
          class="absolute
            inset-0"
          @new-script="openModal('script')"
          @new-chapter="openModal('chapter')"
          @new-character="openModal('character')"
          @import-character="openModal('importChar')"
        />
      </Transition>
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
import { computed, defineComponent, h, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import type { Component } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { MenuPage } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import EditorHeader from '@/components/script-editor/panels/EditorHeader.vue'
import ScriptListPanel from '@/components/script-editor/panels/ScriptListPanel.vue'
import FlowTab from '@/components/script-editor/tabs/FlowTab.vue'
import ConfigTab from '@/components/script-editor/tabs/ConfigTab.vue'
import CharactersTab from '@/components/script-editor/tabs/CharactersTab.vue'
import AssetsTab from '@/components/script-editor/tabs/AssetsTab.vue'
import ValidateTab from '@/components/script-editor/tabs/ValidateTab.vue'
import EditorModals from '@/components/script-editor/modals/EditorModals.vue'
import ShortcutHelpPanel from '@/components/script-editor/panels/ShortcutHelpPanel.vue'
import AgentChatPanel from '@/components/script-editor/agent/AgentChatPanel.vue'
import AgentSettingsPanel from '@/components/script-editor/agent/AgentSettingsPanel.vue'
import AppearanceTab from '@/components/script-editor/tabs/AppearanceTab.vue'
import PreviewStage from '@/components/script-editor/preview/PreviewStage.vue'
import { eventQueue } from '@/core/events/event-queue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { useSettingsStore } from '@/stores/modules/settings'
import type { ShortcutAction } from '@/utils/shortcuts'
import { bindingMatches } from '@/utils/shortcuts'
// 默认背景与主菜单同款。Git LFS 资产：无 LFS 环境读出来是 131 字节指针，
// 背景图加载失败会露出渐变兜底层（见模板注释），构建产物则是真实图。
import defaultBg from '@/assets/images/background2.png'

const { t } = useI18n()
const router = useRouter()
const store = useScriptEditorStore()

/** 编辑器背景图源：自定义时用 asset URL，默认时用内置背景图。
 *  追加 `?v=` 版本号做缓存击穿：同扩展名覆盖上传时 asset URL 不变，
 *  asset protocol 无缓存头，WebView 启发式缓存会显示旧图，版本号强制重新请求。 */
const bgImageSrc = computed(() =>
  store.editorBg.path ? `${convertFileSrc(store.editorBg.path)}?v=${store.bgVersion}` : defaultBg,
)

// ---- 弹窗 ----
const modal = ref<'script' | 'chapter' | 'character' | 'importChar' | null>(null)
const openModal = (which: 'script' | 'chapter' | 'character' | 'importChar') => {
  modal.value = which
}

// ---- Tab 切换（复用设置面板的推入推出过渡） ----
type TabKey =
  | 'flow'
  | 'config'
  | 'characters'
  | 'assets'
  | 'validate'
  | 'agent-chat'
  | 'agent-settings'
  | 'appearance'

/** Tab 顺序（与 EditorHeader 导航一致），用于计算切换方向 */
const TABS: TabKey[] = [
  'flow',
  'config',
  'characters',
  'assets',
  'validate',
  'agent-chat',
  'agent-settings',
  'appearance',
]

// component :is 需要单组件，三个非 MenuPage 根的 Tab 在此保留原有外层布局：
// agent-chat 的居中容器；agent-settings / appearance 的 MenuPage 包装
const agentChatTab = defineComponent({
  name: 'AgentChatTab',
  render: () =>
    h('div', { class: 'flex w-[96%] min-h-0 flex-1 gap-5 mx-auto px-3 py-4' }, h(AgentChatPanel)),
})
const menuTab = (inner: Component) =>
  defineComponent({
    name: 'MenuTab',
    render: () => h(MenuPage, null, () => h(inner)),
  })

const tabComponents: Record<TabKey, Component> = {
  flow: FlowTab, // 实际按 detail 动态切换，见 currentTabComponent
  config: ConfigTab,
  characters: CharactersTab,
  assets: AssetsTab,
  validate: ValidateTab,
  'agent-chat': agentChatTab,
  'agent-settings': menuTab(AgentSettingsPanel),
  appearance: menuTab(AppearanceTab),
}

/** flow Tab 在无剧本时是剧本列表、有剧本时是章节流程，需按 detail 动态取组件 */
const currentTabComponent = computed<Component>(() => {
  if (store.tab === 'flow') return store.detail ? FlowTab : ScriptListPanel
  return tabComponents[store.tab]
})

/**  flow 分支的 key：detail + level 两维都要参与，否则 KeepAlive 缓存键会撞车。*/
const tabKey = computed<string>(() => {
  if (store.tab !== 'flow') return store.tab
  if (!store.detail) return 'flow-list' // 剧本列表
  return store.level === 'chapter' ? 'flow-chapter' : 'flow-chapters' // FlowTab：按 level 区分
})

/** 转场方向：前进 → slide-left（新页从右进），后退 → slide-right；首尾 wrap 视为前进 */
const transitionName = ref<'slide-left' | 'slide-right'>('slide-left')
watch(
  () => store.tab,
  (newTab, oldTab) => {
    console.log('切换 tab', newTab, oldTab)
    if (!oldTab) return
    const prevIdx = TABS.indexOf(oldTab as TabKey)
    const nextIdx = TABS.indexOf(newTab as TabKey)
    if (prevIdx < 0 || nextIdx < 0) return
    const forward = nextIdx > prevIdx || (prevIdx === TABS.length - 1 && nextIdx === 0)
    transitionName.value = forward ? 'slide-left' : 'slide-right'
  },
)

const shouldAnimate = computed(() => {
  // 当显示 ScriptListPanel 时禁用动画
  console.log(
    'store的detail是',
    store.detail,
    '是否为空',
    store.detail === undefined || store.detail === null,
  )
  if (store.tab === 'flow' && store.detail) {
    return false
  }
  return true
})

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

// ---- 快捷键（键位可自定义，见 ShortcutHelpPanel / settings.shortcuts） ----
const settings = useSettingsStore()

const onKey = (e: KeyboardEvent) => {
  // 在输入框里让位给浏览器原生行为，否则作者想撤销一个词却把整个事件列表
  // 回退了一帧，而且刚敲的字（还没 change 提交）会一起消失。
  const el = e.target as HTMLElement | null
  const typing =
    !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)

  const hit = (action: ShortcutAction) => bindingMatches(settings.shortcuts[action], e)

  // Esc 与快捷键表切换不带修饰键且语义特殊，先行处理
  if (hit('esc')) {
    if (store.previewing) {
      void store.stopPreview()
    } else if (shortcutHelp.value) {
      shortcutHelp.value = false
    } else if (store.level === 'chapter') {
      store.backToFlow()
    }
    return
  }
  if (!typing && hit('shortcutHelp')) {
    e.preventDefault()
    shortcutHelp.value = !shortcutHelp.value
    return
  }

  // 试玩期间键盘归游戏，编辑器不抢
  if (store.previewing) return

  // 保存允许在输入框里触发（写作中随手保存）
  if (hit('save')) {
    e.preventDefault()
    void store.save()
    return
  }
  if (typing) return

  if (hit('redo')) {
    e.preventDefault()
    store.redo()
    return
  }
  if (hit('undo')) {
    e.preventDefault()
    store.undo()
    return
  }
  if (hit('copyEvent')) {
    e.preventDefault()
    if (store.chapter) store.duplicateEvent(store.selectedEvent)
    return
  }
  if (hit('playtest')) {
    e.preventDefault()
    void playtest()
    return
  }
  // 展开/收起事件属性栏（章节编辑页才有意义）
  if (hit('expandProps') && store.level === 'chapter') {
    e.preventDefault()
    store.propsExpanded = !store.propsExpanded
    return
  }

  // 以下都只在章节编辑页有意义
  if (store.level !== 'chapter' || !store.chapter) return
  const last = store.chapter.events.length - 1

  if (hit('deleteEvent')) {
    e.preventDefault()
    store.removeEvent(store.selectedEvent)
  } else if (hit('moveCursor') || hit('moveEvent')) {
    e.preventDefault()
    const step = e.key === 'ArrowUp' ? -1 : 1
    const to = store.selectedEvent + step
    if (to < 0 || to > last) return
    if (hit('moveEvent')) store.moveEvent(store.selectedEvent, to)
    else store.selectedEvent = to
  }
}

onMounted(async () => {
  window.addEventListener('keydown', onKey)
  // 快捷键持久化数据可能被旧版捕获逻辑写坏（如 Ctrl+S 被绑成单独 S），启动时校验回退
  settings.ensureValidShortcuts()
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
  overflow: hidden;
  /* 渐变兜底：背景图缺失（无 LFS 的本地环境）或加载失败时露出的底色，
     与改造前的实现一致 */
  background:
    radial-gradient(900px 500px at 78% 12%, rgba(121, 217, 255, 0.1), transparent 62%),
    radial-gradient(700px 600px at 15% 88%, rgba(90, 140, 190, 0.12), transparent 64%),
    linear-gradient(168deg, #101a26 0%, #16202c 45%, #1b2430 100%);
}
.bg-image {
  position: absolute;
  inset: 0;
  background-size: cover;
  background-position: center;
  /* 默认背景图亮度过高（主菜单那张，亮度均值 ~211/255），直接铺白字没法读，
     固定压暗再叠加可调模糊；模糊半径由滑块写入 --bg-blur */
  filter: brightness(0.35) blur(var(--bg-blur, 12px));
  /* 放大一点，避免模糊后四周露出透明边缘 */
  transform: scale(1.02);
}
.bg-dim-mask {
  position: absolute;
  inset: 0;
  /* 压暗遮罩不随图模糊；不透明度由滑块写入 --bg-dim */
  background: rgba(0, 0, 0, var(--bg-dim, 0.3));
}
.editor-root > *:not(.bg-layer) {
  position: relative;
  z-index: 1;
}

/* ========== Tab 切换推入推出过渡（与设置面板同一套动画） ==========
 * 只用 transform、不用 opacity：编辑器背景层带 blur 滤镜，动画里叠加
 * 透明度变化会让 WebView 合成器在滤镜层上重绘，输入框区域会闪白
 * （设置面板没有 blur 背景层，不受影响）。
 */
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition: transform 0.32s cubic-bezier(0.32, 0.72, 0, 1);
}

/* 左滑 → 下一项：新页从右侧推入，旧页向左滑出 */
.slide-left-enter-from {
  transform: translateX(100%);
}
.slide-left-leave-to {
  transform: translateX(-150%);
}

/* 右滑 → 上一项：新页从左侧推入，旧页向右滑出 */
.slide-right-enter-from {
  transform: translateX(-100%);
}
.slide-right-leave-to {
  transform: translateX(150%);
}
</style>
