<template>
  <div class="relative h-full w-full overflow-hidden">
    <!-- 顶栏：与 SettingsNav 同构 -->
    <div class="snav">
      <img
        src="@/assets/images/LingChatLogo.png"
        alt="LingChat"
        class="ml-5 hidden w-20 xl:block"
      />
      <nav
        ref="navEl"
        class="relative flex h-full w-full flex-nowrap items-center justify-center gap-1
          overflow-x-auto overflow-y-hidden px-2"
      >
        <div
          ref="indicatorEl"
          class="absolute bottom-0 left-0 h-1 w-0 rounded bg-brand z-10
            shadow-[0_0_10px_rgba(121,217,255,0.4)]"
        ></div>
        <Button
          v-for="t in tabs"
          :key="t.key"
          :ref="(el: unknown) => setTabRef(t.key, el)"
          type="nav"
          :icon="t.icon"
          :active="store.tab === t.key"
          :disabled="!store.detail && t.key !== 'flow'"
          @click="switchTab(t.key)"
        >
          <p class="hidden xl:block">{{ t.label }}</p>
        </Button>
      </nav>
      <Icon
        icon="close"
        :size="40"
        class="flex cursor-pointer items-center justify-center rounded-full border-none
          bg-transparent p-1.5 text-white transition-all duration-300 ease-in-out
          hover:rotate-90 hover:bg-white/10 hover:text-accent"
        @click="leave"
      />
    </div>

    <!-- 面包屑 -->
    <div class="crumb">
      <button
        v-if="store.detail"
        class="text-brand hover:underline"
        @click="store.closeScript()"
      >
        ‹ 剧本列表
      </button>
      <span v-else>剧本编辑器</span>

      <template v-if="store.detail">
        <span class="opacity-40">›</span>
        <button
          v-if="store.level === 'chapter'"
          class="text-brand hover:underline"
          @click="store.backToFlow()"
        >
          {{ store.detail.package.scriptName }}
        </button>
        <b v-else>{{ store.detail.package.scriptName }}</b>

        <template v-if="store.level === 'chapter' && store.chapter">
          <span class="opacity-40">›</span>
          <b>{{ store.chapter.name || store.chapter.id }}</b>
          <span class="text-xs opacity-30">{{ store.chapter.id }}.yaml</span>
        </template>
      </template>

      <span class="ml-auto flex items-center gap-3">
        <span
          v-if="store.detail"
          class="save-state"
        >
          <i :class="store.dirty ? 'bg-yellow-300' : 'bg-green-400'"></i>
          {{ saveLabel }}
        </span>
        <button
          v-if="store.level === 'chapter'"
          class="chip"
          :disabled="!store.canUndo"
          @click="store.undo()"
        >
          撤销
        </button>
        <button
          v-if="store.level === 'chapter'"
          class="chip"
          :disabled="!store.canRedo"
          @click="store.redo()"
        >
          重做
        </button>
        <button
          v-if="store.detail"
          class="chip"
          @click="playtest"
        >
          试玩
        </button>
      </span>
    </div>

    <!-- 主体 -->
    <div class="body">
      <!-- 剧本列表 -->
      <MenuPage v-if="!store.detail">
        <MenuItem title="选择要编辑的剧本">
          <template #header>
            <Icon
              icon="package"
              :size="20"
            />
          </template>

          <p
            v-if="store.loading"
            class="py-8 text-center text-white/50"
          >
            正在读取…
          </p>
          <p
            v-else-if="store.scripts.length === 0"
            class="py-8 text-center text-white/50"
          >
            还没有任何剧本，点下面新建一个
          </p>

          <div
            v-for="s in store.scripts"
            :key="s.key"
            class="script-card"
            @click="store.openScript(s.key)"
          >
            <div class="flex items-baseline gap-2">
              <span class="font-semibold text-white">{{ s.scriptName }}</span>
              <span
                v-if="s.isAdventure"
                class="tag"
                >羁绊冒险</span
              >
              <span
                v-if="!s.loadedByEngine"
                class="tag tag-warn"
                >未加载</span
              >
              <span class="ml-auto text-xs text-white/40">{{ s.chapterCount }} 章</span>
            </div>
            <p class="mt-1 text-xs text-white/50">{{ s.description || '（没有简介）' }}</p>
            <p class="mt-1 font-mono text-[10px] text-white/25">{{ s.key }}</p>
          </div>

          <Button
            type="big"
            class="mt-4"
            @click="createOpen = true"
          >
            ＋ 新建剧本
          </Button>
        </MenuItem>
      </MenuPage>

      <!-- 章节流程 -->
      <MenuPage v-else-if="store.tab === 'flow' && store.level === 'flow'">
        <MenuItem title="章节流程">
          <template #header>
            <Icon
              icon="adventure"
              :size="20"
            />
          </template>
          <div class="mb-3 flex flex-wrap gap-2">
            <button
              class="chip"
              @click="newChapterOpen = true"
            >
              ＋ 新建章节
            </button>
            <button
              class="chip"
              @click="store.runValidation()"
            >
              校验剧本
            </button>
            <button
              class="chip"
              @click="openFolder"
            >
              打开剧本目录
            </button>
          </div>
          <ChapterFlow />
        </MenuItem>
      </MenuPage>

      <!-- 章节编辑 -->
      <div
        v-else-if="store.tab === 'flow' && store.level === 'chapter'"
        class="editcols"
      >
        <div class="col-tl">
          <MenuItem
            title="事件时间线"
            class="fill"
          >
            <template #header>
              <Icon
                icon="text"
                :size="20"
              />
            </template>
            <div class="mb-2 flex items-center gap-2">
              <input
                class="glass-input flex-1"
                placeholder="章节显示名（留空则用文件名）"
                :value="store.chapter?.name ?? ''"
                @change="onRename"
              />
              <label class="flex shrink-0 items-center gap-1.5 text-xs whitespace-nowrap text-white/60">
                <input
                  type="checkbox"
                  :checked="store.foldCompounds"
                  @change="store.foldCompounds = !store.foldCompounds"
                />
                折叠固定套路
              </label>
              <span class="shrink-0 text-xs text-white/40">
                {{ store.chapter?.events.length ?? 0 }} 个事件
              </span>
            </div>
            <div class="scroll-body">
              <ChapterTimeline />
            </div>
          </MenuItem>
        </div>

        <div class="col-pr">
          <MenuItem
            title="事件属性"
            class="fill"
          >
            <template #header>
              <Icon
                icon="setting"
                :size="20"
              />
            </template>
            <div class="scroll-body">
              <EventPropertyPanel />
            </div>
          </MenuItem>
        </div>
      </div>

      <!-- 其余 tab -->
      <MenuPage v-else>
        <MenuItem :title="currentTabLabel">
          <template #header>
            <Icon
              icon="box"
              :size="20"
            />
          </template>
          <p class="py-6 text-sm leading-relaxed text-white/55">
            {{ placeholderText }}
          </p>
        </MenuItem>
      </MenuPage>
    </div>

    <!-- 校验抽屉 -->
    <Transition name="drawer">
      <div
        v-if="store.validationOpen && store.report"
        class="vdrawer"
      >
        <div class="vhead">
          <span class="text-white">剧本校验</span>
          <span class="text-red-300">{{ store.report.errorCount }} 错误</span>
          <span class="text-yellow-200">{{ store.report.warnCount }} 警告</span>
          <span class="text-white/40">{{ store.report.infoCount }} 提示</span>
          <button
            class="ml-auto text-white/40 hover:text-white"
            @click="store.validationOpen = false"
          >
            ✕
          </button>
        </div>
        <div class="vlist">
          <p
            v-if="store.report.diagnostics.length === 0"
            class="py-6 text-center text-sm text-white/45"
          >
            没有发现问题
          </p>
          <div
            v-for="(d, i) in store.report.diagnostics"
            :key="i"
            class="vitem"
            :style="{ borderLeftColor: barColor(d.severity) }"
            @click="jumpTo(d)"
          >
            <div class="text-xs leading-relaxed text-white/85">{{ d.message }}</div>
            <div class="mt-0.5 font-mono text-[10px] text-white/35">
              {{ locationOf(d) }}
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <button
      v-if="store.report && !store.validationOpen"
      class="vfab"
      @click="store.validationOpen = true"
    >
      校验：{{ store.report.errorCount }} 错误 · {{ store.report.warnCount }} 警告
    </button>

    <!-- 新建剧本 -->
    <BaseModal
      :show="createOpen"
      title="新建剧本"
      @close="createOpen = false"
      @confirm="doCreateScript"
    >
      <div class="flex flex-col gap-3">
        <label class="text-sm text-brand">剧本名</label>
        <input
          v-model="form.folderName"
          class="glass-input"
          placeholder="例如：一起看星星"
        />
        <label class="text-sm text-brand">简介</label>
        <textarea
          v-model="form.description"
          class="glass-input min-h-16"
        ></textarea>
        <label class="flex items-center gap-2 text-sm text-white/70">
          <input
            v-model="form.isAdventure"
            type="checkbox"
          />
          这是某个角色的羁绊冒险
        </label>
        <template v-if="form.isAdventure">
          <label class="text-sm text-brand">绑定角色的目录名</label>
          <input
            v-model="form.boundCharacterFolder"
            class="glass-input"
            placeholder="game_data/characters/ 下的目录名"
          />
        </template>
        <p class="text-xs text-white/40">
          开场章节会自动创建为 main，目录结构和 story_config.yaml 都由编辑器生成。
        </p>
      </div>
    </BaseModal>

    <!-- 新建章节 -->
    <BaseModal
      :show="newChapterOpen"
      title="新建章节"
      @close="newChapterOpen = false"
      @confirm="doCreateChapter"
    >
      <div class="flex flex-col gap-3">
        <label class="text-sm text-brand">章节文件名</label>
        <input
          v-model="chapterForm.id"
          class="glass-input"
          placeholder="main2，或 Intro/intro2 放进子目录"
        />
        <label class="text-sm text-brand">显示名</label>
        <input
          v-model="chapterForm.name"
          class="glass-input"
          placeholder="例如：2 樱花的公园"
        />
        <p class="text-xs text-white/40">
          新章节自带一条「章节结束」，免得一保存就报缺少结束事件。
        </p>
      </div>
    </BaseModal>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { Button, Icon } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
// BaseModal 没有在 ui/index.ts 里导出，直接按路径引
import BaseModal from '@/components/ui/BaseModal.vue'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { createScript, openScriptFolder } from '@/api/services/script-editor'
import type { Diagnostic } from '@/api/services/script-editor'
import ChapterFlow from '@/components/script-editor/ChapterFlow.vue'
import ChapterTimeline from '@/components/script-editor/ChapterTimeline.vue'
import EventPropertyPanel from '@/components/script-editor/EventPropertyPanel.vue'

const router = useRouter()
const store = useScriptEditorStore()

type TabKey = 'flow' | 'config' | 'characters' | 'assets' | 'validate'

const tabs: { key: TabKey; label: string; icon: 'adventure' | 'setting' | 'character' | 'background' | 'achievement' }[] = [
  { key: 'flow', label: '章节流程', icon: 'adventure' },
  { key: 'config', label: '剧本设置', icon: 'setting' },
  { key: 'characters', label: '角色', icon: 'character' },
  { key: 'assets', label: '素材', icon: 'background' },
  { key: 'validate', label: '校验', icon: 'achievement' },
]

// ---- nav 指示条（与 SettingsNav 同一套做法）----
const navEl = ref<HTMLElement | null>(null)
const indicatorEl = ref<HTMLElement | null>(null)
const tabRefs: Record<string, HTMLElement | null> = {}

const setTabRef = (key: string, el: unknown) => {
  const inst = el as { $el?: HTMLElement } | null
  tabRefs[key] = inst?.$el ?? null
}

const moveIndicator = async () => {
  await nextTick()
  const target = tabRefs[store.tab]
  if (!indicatorEl.value || !target) return
  indicatorEl.value.style.transition =
    'left 0.3s cubic-bezier(0.18, 0.89, 0.32, 1), width 0.3s cubic-bezier(0.18, 0.89, 0.32, 1)'
  indicatorEl.value.style.left = `${target.offsetLeft}px`
  indicatorEl.value.style.width = `${target.offsetWidth}px`
}

const switchTab = (key: TabKey) => {
  if (!store.detail && key !== 'flow') return
  store.tab = key
  if (key === 'validate') void store.runValidation()
  void moveIndicator()
}

watch(() => store.tab, moveIndicator)
watch(() => store.detail?.package.key, moveIndicator)

// ---- 面包屑状态 ----
const saveLabel = computed(() => {
  if (store.saving) return '正在保存…'
  if (store.dirty) return '有未保存改动'
  if (store.lastSavedAt) {
    const d = new Date(store.lastSavedAt)
    return `已自动保存 · ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
  }
  return '已保存'
})

const currentTabLabel = computed(() => tabs.find((t) => t.key === store.tab)?.label ?? '')

const placeholderText = computed(() => {
  switch (store.tab) {
    case 'config':
      return '剧本设置（名称、简介、开场章节、羁绊冒险配置）的表单在后续迭代接上；后端命令 editor_write_story_config 已就绪。注意：改写 story_config.yaml 会丢掉文件里的 YAML 注释，保存前会自动留一份 .bak。'
    case 'characters':
      return `这个剧本有 ${store.characters.length} 个剧本内角色。创建角色的后端命令 editor_create_character 已就绪，表单在后续迭代接上。`
    case 'assets':
      return '素材可以直接在事件属性面板里点「导入…」按需导入，会落到引擎第一个候选目录，保证一定能被找到。集中管理的素材页在后续迭代接上。'
    default:
      return '校验结果显示在右下角的抽屉里。'
  }
})

// ---- 表单 ----
const createOpen = ref(false)
const newChapterOpen = ref(false)

const form = reactive({
  folderName: '',
  description: '',
  isAdventure: false,
  boundCharacterFolder: '',
})

const chapterForm = reactive({ id: '', name: '' })

const doCreateScript = async () => {
  createOpen.value = false
  try {
    const pkg = await createScript({
      folderName: form.folderName,
      description: form.description,
      isAdventure: form.isAdventure,
      boundCharacterFolder: form.boundCharacterFolder,
    })
    form.folderName = ''
    form.description = ''
    form.isAdventure = false
    form.boundCharacterFolder = ''
    await store.refreshScripts()
    await store.openScript(pkg.key)
  } catch (e) {
    store.notifyError('新建剧本失败', e)
  }
}

const doCreateChapter = async () => {
  newChapterOpen.value = false
  await store.createChapter(chapterForm.id, chapterForm.name)
  chapterForm.id = ''
  chapterForm.name = ''
}

const onRename = (e: Event) => store.setChapterName((e.target as HTMLInputElement).value)

const openFolder = async () => {
  if (!store.scriptKey) return
  try {
    await openScriptFolder(store.scriptKey)
  } catch (err) {
    store.notifyError('打开目录失败', err)
  }
}

const playtest = async () => {
  const ok = await store.preparePlaytest()
  if (!ok) return
  store.notifyOk('剧本已重新加载', '回到主菜单的剧情模式即可试玩')
}

const leave = async () => {
  await store.save()
  void router.push('/')
}

// ---- 诊断跳转 ----
const barColor = (s: string) =>
  s === 'error' ? '#f87171' : s === 'warn' ? '#fbbf24' : 'rgba(255,255,255,.25)'

const locationOf = (d: Diagnostic) => {
  if (!d.chapter) return 'story_config.yaml'
  const ev = d.eventIndex !== undefined ? ` · 第 ${d.eventIndex + 1} 个事件` : ''
  return `${d.chapter}.yaml${ev}`
}

const jumpTo = async (d: Diagnostic) => {
  if (!d.chapter) {
    store.tab = 'config'
    return
  }
  store.tab = 'flow'
  if (store.chapter?.id !== d.chapter) await store.openChapter(d.chapter)
  if (d.eventIndex !== undefined) store.selectedEvent = d.eventIndex
}

// ---- 快捷键 ----
const onKey = (e: KeyboardEvent) => {
  const mod = e.ctrlKey || e.metaKey
  if (!mod) return
  const k = e.key.toLowerCase()
  if (k === 's') {
    e.preventDefault()
    void store.save()
  } else if (k === 'z' && !e.shiftKey) {
    e.preventDefault()
    store.undo()
  } else if ((k === 'z' && e.shiftKey) || k === 'y') {
    e.preventDefault()
    store.redo()
  }
}

onMounted(async () => {
  window.addEventListener('keydown', onKey)
  await store.init()
  await moveIndicator()
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKey)
  void store.save()
})
</script>

<style scoped>
@reference "tailwindcss";

.snav {
  position: relative;
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  padding: 0.5rem 1.25rem;
}

.crumb {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0 2rem 0.25rem;
  font-size: 0.8rem;
  color: rgba(255, 255, 255, 0.55);
}
.crumb b {
  font-weight: 600;
  color: #fff;
}

.save-state {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 0.75rem;
  color: rgba(255, 255, 255, 0.5);
}
.save-state i {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.body {
  display: flex;
  height: calc(100% - 5.5rem);
  min-height: 0;
  flex-direction: column;
}

.editcols {
  display: flex;
  width: 94%;
  min-height: 0;
  flex: 1;
  gap: 1.25rem;
  margin: 0 auto;
  padding: 1rem 0.75rem;
}
.col-tl {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
}
.col-pr {
  display: flex;
  min-height: 0;
  flex: 0 0 340px;
  flex-direction: column;
}
.fill {
  display: flex;
  height: 100%;
  min-height: 0;
  flex-direction: column;
}
.scroll-body {
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
}

.script-card {
  width: 100%;
  cursor: pointer;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  padding: 11px 13px;
  margin-bottom: 8px;
  background: rgba(255, 255, 255, 0.06);
  transition: all 0.2s;
}
.script-card:hover {
  border-color: var(--accent-color);
  background: rgba(121, 217, 255, 0.08);
}

.tag {
  border: 1px solid rgba(121, 217, 255, 0.35);
  border-radius: 99px;
  padding: 0 7px;
  font-size: 0.62rem;
  color: var(--accent-color);
  background: rgba(121, 217, 255, 0.12);
}
.tag-warn {
  color: #fcd34d;
  border-color: rgba(251, 191, 36, 0.35);
  background: rgba(251, 191, 36, 0.12);
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 0.5rem;
  padding: 0.3rem 0.75rem;
  font-size: 0.8rem;
  white-space: nowrap;
  color: rgba(255, 255, 255, 0.7);
  background: rgba(255, 255, 255, 0.06);
  transition: all 0.2s;
}
.chip:hover:not(:disabled) {
  color: #fff;
  background: rgba(255, 255, 255, 0.12);
}
.chip:disabled {
  cursor: not-allowed;
  opacity: 0.4;
}

.glass-input {
  @apply w-full rounded-lg border border-white/10 bg-white/10 px-3 py-2.5 text-sm text-white
    backdrop-blur-xl backdrop-saturate-150 transition-all duration-200
    focus:border-brand focus:ring-2 focus:ring-brand/20 focus:outline-none;
}

.vdrawer {
  position: absolute;
  right: 0;
  bottom: 0;
  z-index: 40;
  display: flex;
  width: 360px;
  max-height: 62%;
  flex-direction: column;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
  border-left: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px 0 0 0;
  background: rgba(11, 15, 20, 0.96);
  backdrop-filter: blur(14px);
  box-shadow: -8px -8px 30px rgba(0, 0, 0, 0.35);
}
.vhead {
  display: flex;
  align-items: center;
  gap: 8px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  padding: 9px 13px;
  font-size: 0.75rem;
}
.vlist {
  overflow-y: auto;
  padding: 8px;
}
.vitem {
  margin-bottom: 5px;
  cursor: pointer;
  border-left: 2px solid;
  border-radius: 7px;
  padding: 8px 10px;
  background: rgba(255, 255, 255, 0.05);
  transition: background 0.15s;
}
.vitem:hover {
  background: rgba(255, 255, 255, 0.1);
}

.vfab {
  position: absolute;
  right: 18px;
  bottom: 18px;
  z-index: 39;
  border: 1px solid rgba(248, 113, 113, 0.4);
  border-radius: 20px;
  padding: 8px 14px;
  font-size: 0.75rem;
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.14);
  backdrop-filter: blur(10px);
}
.vfab:hover {
  background: rgba(248, 113, 113, 0.22);
}

.drawer-enter-active,
.drawer-leave-active {
  transition: all 0.25s cubic-bezier(0.18, 0.89, 0.32, 1);
}
.drawer-enter-from,
.drawer-leave-to {
  opacity: 0;
  transform: translateY(12px);
}
</style>
