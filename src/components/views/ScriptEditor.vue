<template>
  <!--
    背景层必须自己造。窗口是 transparent: true（tauri.conf.json），设置面板之所以
    能透出画面是因为它盖在 MainChat 上；/script-editor 是独立路由，底下什么都没有，
    不给背景就直接透出桌面。Credits.vue 同理显式加了 bg-[#0a0a0c]。
    这里用渐变而不是背景图，避免依赖 Git LFS 资源。
  -->
  <div class="editor-root">
    <div class="bg-layer"></div>

    <!-- 顶栏：与 SettingsNav 同构 -->
    <div class="snav">
      <span class="logo">LingChat</span>
      <nav ref="navEl">
        <div
          ref="indicatorEl"
          class="indicator"
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
          <span
            v-if="t.key === 'validate' && store.report && store.report.errorCount > 0"
            class="nav-badge"
            >{{ store.report.errorCount }}</span
          >
        </Button>
      </nav>
      <Icon
        icon="close"
        :size="40"
        class="close-btn"
        @click="leave"
      />
    </div>

    <!-- 面包屑 -->
    <div class="crumb">
      <button
        v-if="store.detail"
        class="link"
        @click="store.closeScript()"
      >
        ‹ 剧本列表
      </button>
      <span v-else>剧本编辑器</span>

      <template v-if="store.detail">
        <span class="sep">›</span>
        <button
          v-if="store.level === 'chapter'"
          class="link"
          @click="store.backToFlow()"
        >
          {{ store.detail.package.scriptName }}
        </button>
        <b v-else>{{ store.detail.package.scriptName }}</b>

        <template v-if="store.level === 'chapter' && store.chapter">
          <span class="sep">›</span>
          <b>{{ store.chapter.name || store.chapter.id }}</b>
          <span class="dim">{{ store.chapter.id }}.yaml</span>
        </template>
      </template>

      <span class="right">
        <span
          v-if="store.detail"
          class="save-state"
        >
          <i :class="store.dirty ? 'pending' : 'clean'"></i>
          {{ saveLabel }}
        </span>
        <template v-if="store.level === 'chapter'">
          <button
            class="chip"
            :disabled="!store.canUndo"
            @click="store.undo()"
          >
            撤销
          </button>
          <button
            class="chip"
            :disabled="!store.canRedo"
            @click="store.redo()"
          >
            重做
          </button>
        </template>
        <button
          v-if="store.detail"
          class="chip primary"
          @click="playtest"
        >
          {{ store.level === 'chapter' ? '从本章试玩' : '从开场试玩' }}
        </button>
      </span>
    </div>

    <!-- 主体 -->
    <div class="body">
      <!-- ============ 剧本列表 ============ -->
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
            class="empty"
          >
            正在读取…
          </p>
          <p
            v-else-if="store.scripts.length === 0"
            class="empty"
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
                class="tag warn"
                >未加载</span
              >
              <span class="ml-auto text-xs text-white/40">{{ s.chapterCount }} 章</span>
              <button
                class="card-del"
                title="删除剧本（移到回收目录）"
                @click.stop="store.deleteScript(s.key, s.scriptName)"
              >
                ✕
              </button>
            </div>
            <p class="mt-1 text-xs text-white/50">{{ s.description || '（没有简介）' }}</p>
            <p class="mt-1 font-mono text-[10px] text-white/25">{{ s.key }}</p>
          </div>

          <Button
            type="big"
            class="mt-4"
            @click="modal = 'script'"
          >
            ＋ 新建剧本
          </Button>
        </MenuItem>
      </MenuPage>

      <!-- ============ 章节流程 ============ -->
      <MenuPage v-else-if="store.tab === 'flow' && store.level === 'flow'">
        <MenuItem title="章节流程">
          <template #header>
            <Icon
              icon="adventure"
              :size="20"
            />
          </template>
          <div class="toolbar">
            <button
              class="chip"
              @click="modal = 'chapter'"
            >
              ＋ 新建章节
            </button>
            <button
              class="chip"
              @click="store.runValidation()"
            >
              重新校验
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

      <!-- ============ 章节编辑 ============ -->
      <div
        v-else-if="store.tab === 'flow' && store.level === 'chapter'"
        class="editcols"
      >
        <div class="col-main">
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
              <label class="inline-toggle">
                <Toggle
                  :checked="store.foldCompounds"
                  @change="(v: boolean) => (store.foldCompounds = v)"
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

        <div class="col-side">
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

      <!-- ============ 剧本设置 ============ -->
      <MenuPage v-else-if="store.tab === 'config'">
        <MenuItem title="剧本设置">
          <template #header>
            <Icon
              icon="setting"
              :size="20"
            />
          </template>

          <p class="notice">
            改写 <code>story_config.yaml</code> 会丢掉文件里的 YAML 注释（六个官方剧本的
            config 都带中文注释）。保存前会自动留一份 <code>.bak</code>。
          </p>

          <div
            v-for="f in store.schema?.storyConfigFields ?? []"
            :key="f.key"
            class="field"
          >
            <label class="f-label">
              {{ f.label }}<span
                v-if="f.required"
                class="req"
                >＊</span
              >
            </label>
            <p class="f-key">{{ f.key }}</p>
            <select
              v-if="f.kind === 'chapter'"
              class="glass-input"
              :value="configDraft[f.key] ?? ''"
              @change="(e) => setConfig(f.key, (e.target as HTMLSelectElement).value)"
            >
              <option
                v-for="c in store.chapterOptions.filter((o) => o.value !== 'end')"
                :key="c.value"
                :value="c.value"
              >
                {{ c.label }}
              </option>
            </select>
            <textarea
              v-else-if="f.kind === 'textarea'"
              class="glass-input min-h-16"
              :value="String(configDraft[f.key] ?? '')"
              @change="(e) => setConfig(f.key, (e.target as HTMLTextAreaElement).value)"
            ></textarea>
            <input
              v-else
              class="glass-input"
              :value="String(configDraft[f.key] ?? '')"
              @change="(e) => setConfig(f.key, (e.target as HTMLInputElement).value)"
            />
            <p
              v-if="f.hint"
              class="f-hint"
            >
              {{ f.hint }}
            </p>
          </div>

          <!-- 羁绊冒险 -->
          <div class="section">
            <label class="inline-toggle mb-2">
              <Toggle
                :checked="isAdventure"
                @change="toggleAdventure"
              />
              这是某个角色的羁绊冒险
            </label>
            <template v-if="isAdventure">
              <div class="field">
                <label class="f-label">绑定角色目录名</label>
                <p class="f-key">adventure.bound_character_folder</p>
                <input
                  class="glass-input"
                  :value="adventureField('bound_character_folder')"
                  @change="(e) => onAdventureText('bound_character_folder', e)"
                />
              </div>
              <div class="field">
                <label class="f-label">排序</label>
                <p class="f-key">adventure.order</p>
                <input
                  class="glass-input"
                  type="number"
                  :value="adventureField('order')"
                  @change="(e) => onAdventureNumber('order', e)"
                />
                <p class="f-hint">决定在角色卡里的显示顺序</p>
              </div>
              <p class="f-hint">
                解锁条件（<code>unlock_conditions</code>）目前保持文件里的原值不动，
                下一轮补可视化编辑。<code>trigger.mode</code> 引擎没有任何消费者，
                因此不在这里暴露，但读写时原样保留。
              </p>
            </template>
          </div>

          <Button
            type="big"
            class="mt-4"
            @click="saveConfig"
          >
            保存剧本设置
          </Button>
        </MenuItem>
      </MenuPage>

      <!-- ============ 角色 ============ -->
      <MenuPage v-else-if="store.tab === 'characters'">
        <MenuItem title="剧本内角色">
          <template #header>
            <Icon
              icon="character"
              :size="20"
            />
          </template>

          <p class="notice">
            这些角色只属于本剧本（<code>characters/</code> 目录），随剧本一起分发。
            剧本里用 <code>character: &lt;下面的引用名&gt;</code> 指代他们；
            写 <code>MAIN</code> 表示当前选中的主角。
          </p>

          <p
            v-if="store.characters.length === 0"
            class="empty"
          >
            还没有剧本内角色
          </p>
          <div
            v-for="c in store.characters"
            :key="c.folder"
            class="row-card"
          >
            <div class="flex items-baseline gap-2">
              <span class="font-semibold text-white">{{ c.aiName }}</span>
              <code class="ref">character: {{ c.roleKey }}</code>
              <span class="ml-auto text-xs text-white/40">
                {{ c.emotions.length }} 个表情{{
                  c.clothes.length ? ` · ${c.clothes.length} 套服装` : ''
                }}
              </span>
            </div>
            <p
              v-if="c.emotions.length === 0"
              class="mt-1 text-xs text-yellow-200"
            >
              avatar/ 下没有任何图片，立绘不会显示
            </p>
            <p
              v-else
              class="mt-1 text-xs text-white/40"
            >
              {{ c.emotions.slice(0, 12).join('、') }}{{ c.emotions.length > 12 ? ' …' : '' }}
            </p>
          </div>

          <Button
            type="big"
            class="mt-4"
            @click="modal = 'character'"
          >
            ＋ 新建角色
          </Button>
        </MenuItem>
      </MenuPage>

      <!-- ============ 素材 ============ -->
      <MenuPage v-else-if="store.tab === 'assets'">
        <MenuItem title="素材">
          <template #header>
            <Icon
              icon="background"
              :size="20"
            />
          </template>

          <p class="notice">
            引擎查找素材的顺序是<b>先本剧本，再全局</b>，所以两处都能被找到，区别在于：
            <b>剧本素材</b>随剧本一起分发，别的剧本看不到；<b>全局素材</b>所有剧本共享，
            但导出剧本时不会带走。
          </p>

          <div
            v-for="k in assetKinds"
            :key="k.key"
            class="asset-group"
          >
            <div class="asset-head">
              <span class="asset-title">{{ k.label }}</span>
              <button
                class="chip"
                @click="importAsset(k.key, 'script')"
              >
                导入到本剧本
              </button>
              <button
                class="chip"
                @click="importAsset(k.key, 'global')"
              >
                导入为全局
              </button>
            </div>
            <div class="asset-cols">
              <div>
                <p class="asset-sub">本剧本 · {{ store.assets[k.key].length }}</p>
                <p
                  v-if="store.assets[k.key].length === 0"
                  class="asset-empty"
                >
                  无
                </p>
                <span
                  v-for="n in store.assets[k.key]"
                  :key="n"
                  class="asset-item"
                  >{{ n }}</span
                >
              </div>
              <div>
                <p class="asset-sub">全局 · {{ store.globalAssets[k.key].length }}</p>
                <p
                  v-if="store.globalAssets[k.key].length === 0"
                  class="asset-empty"
                >
                  无
                </p>
                <span
                  v-for="n in store.globalAssets[k.key]"
                  :key="n"
                  class="asset-item global"
                  >{{ n }}</span
                >
              </div>
            </div>
          </div>
        </MenuItem>
      </MenuPage>

      <!-- ============ 校验（整页，不再用抽屉）============ -->
      <MenuPage v-else>
        <MenuItem title="校验">
          <template #header>
            <Icon
              icon="achievement"
              :size="20"
            />
          </template>

          <div class="toolbar">
            <button
              class="chip"
              @click="store.runValidation()"
            >
              重新校验
            </button>
            <span
              v-if="store.report"
              class="counts"
            >
              <b class="err">{{ store.report.errorCount }}</b> 错误 ·
              <b class="warn">{{ store.report.warnCount }}</b> 警告 ·
              <b class="info">{{ store.report.infoCount }}</b> 提示
            </span>
          </div>

          <p
            v-if="!store.report"
            class="empty"
          >
            正在校验…
          </p>
          <p
            v-else-if="store.report.diagnostics.length === 0"
            class="ok-banner"
          >
            没有发现问题，这个剧本可以正常跑起来。
          </p>

          <template v-else>
            <!-- 剧本级问题 -->
            <div
              v-if="store.scriptDiagnostics.length"
              class="diag-group"
            >
              <div class="diag-head">
                <span class="diag-title">剧本整体</span>
                <span class="diag-file">story_config.yaml</span>
              </div>
              <div
                v-for="(d, i) in store.scriptDiagnostics"
                :key="i"
                class="diag"
                :class="d.severity"
              >
                <span class="dot"></span>
                <span class="msg">{{ d.message }}</span>
              </div>
            </div>

            <!-- 按章节聚合，与流程图同样的顺序 -->
            <div
              v-for="c in store.chapters"
              :key="c.id"
              class="diag-group"
              :class="{ clean: !chapterHas(c.id) }"
            >
              <div class="diag-head">
                <span class="diag-title">{{ c.name || c.id }}</span>
                <span class="diag-file">{{ c.id }}.yaml</span>
                <span class="diag-counts">
                  <b
                    v-if="store.diagnosticsByChapter[c.id]?.errors"
                    class="err"
                    >{{ store.diagnosticsByChapter[c.id].errors }} 错误</b
                  >
                  <b
                    v-if="store.diagnosticsByChapter[c.id]?.warns"
                    class="warn"
                    >{{ store.diagnosticsByChapter[c.id].warns }} 警告</b
                  >
                  <b
                    v-if="store.diagnosticsByChapter[c.id]?.infos"
                    class="info"
                    >{{ store.diagnosticsByChapter[c.id].infos }} 提示</b
                  >
                  <span
                    v-if="!chapterHas(c.id)"
                    class="pass"
                    >通过</span
                  >
                </span>
                <button
                  class="chip"
                  @click="store.openChapter(c.id)"
                >
                  打开
                </button>
              </div>

              <div
                v-for="(d, i) in diagnosticsOf(c.id)"
                :key="i"
                class="diag clickable"
                :class="d.severity"
                @click="jumpTo(d)"
              >
                <span class="dot"></span>
                <span class="msg">{{ d.message }}</span>
                <span
                  v-if="d.eventIndex !== undefined"
                  class="loc"
                  >第 {{ d.eventIndex + 1 }} 个事件 →</span
                >
              </div>
            </div>
          </template>
        </MenuItem>
      </MenuPage>
    </div>

    <!-- 试玩层 -->
    <PreviewStage :from-chapter="previewFrom" />

    <!-- ============ 弹窗（自己写，深色）============ -->
    <Teleport to="body">
      <Transition name="modal">
        <div
          v-if="modal"
          class="modal-mask"
          @click.self="modal = null"
        >
          <div class="modal">
            <div class="modal-head">
              <h4>{{ modalTitle }}</h4>
              <button
                class="modal-x"
                @click="modal = null"
              >
                ✕
              </button>
            </div>

            <template v-if="modal === 'script'">
              <div class="field">
                <label class="f-label">剧本名</label>
                <input
                  v-model="scriptForm.folderName"
                  class="glass-input"
                  placeholder="例如：一起看星星"
                />
                <p class="f-hint">同时作为目录名。羁绊冒险用目录名作全局主键，不能重名。</p>
              </div>
              <div class="field">
                <label class="f-label">简介</label>
                <textarea
                  v-model="scriptForm.description"
                  class="glass-input min-h-16"
                ></textarea>
              </div>
              <label class="inline-toggle">
                <Toggle
                  :checked="scriptForm.isAdventure"
                  @change="(v: boolean) => (scriptForm.isAdventure = v)"
                />
                这是某个角色的羁绊冒险
              </label>
              <div
                v-if="scriptForm.isAdventure"
                class="field mt-2"
              >
                <label class="f-label">绑定角色的目录名</label>
                <input
                  v-model="scriptForm.boundCharacterFolder"
                  class="glass-input"
                  placeholder="game_data/characters/ 下的目录名"
                />
              </div>
            </template>

            <template v-else-if="modal === 'chapter'">
              <div class="field">
                <label class="f-label">章节文件名</label>
                <input
                  v-model="chapterForm.id"
                  class="glass-input"
                  placeholder="main2，或 Intro/intro2 放进子目录"
                />
              </div>
              <div class="field">
                <label class="f-label">显示名</label>
                <input
                  v-model="chapterForm.name"
                  class="glass-input"
                  placeholder="例如：2 樱花的公园"
                />
                <p class="f-hint">新章节自带一条「章节结束」，免得一保存就报缺少结束事件。</p>
              </div>
            </template>

            <template v-else>
              <div class="field">
                <label class="f-label">角色目录名</label>
                <input
                  v-model="charForm.folder"
                  class="glass-input"
                  placeholder="剧本里会写 character: 这个名字"
                />
              </div>
              <div class="field">
                <label class="f-label">显示名</label>
                <input
                  v-model="charForm.aiName"
                  class="glass-input"
                />
              </div>
              <div class="field">
                <label class="f-label">人物设定</label>
                <textarea
                  v-model="charForm.systemPrompt"
                  class="glass-input min-h-24"
                  placeholder="这个角色的性格、说话方式、与主角的关系…"
                ></textarea>
              </div>
              <p class="f-hint">
                创建后请把立绘放进 <code>characters/&lt;目录名&gt;/avatar/</code>，
                文件名用情绪名（如 <code>正常.png</code>）。
              </p>
            </template>

            <div class="modal-foot">
              <button
                class="chip"
                @click="modal = null"
              >
                取消
              </button>
              <button
                class="chip primary"
                @click="confirmModal"
              >
                确定
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { Button, Icon, Toggle } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { createScript, openScriptFolder } from '@/api/services/script-editor'
import type { AssetKind, AssetScope, Diagnostic } from '@/api/services/script-editor'
import ChapterFlow from '@/components/script-editor/ChapterFlow.vue'
import ChapterTimeline from '@/components/script-editor/ChapterTimeline.vue'
import EventPropertyPanel from '@/components/script-editor/EventPropertyPanel.vue'
import PreviewStage from '@/components/script-editor/PreviewStage.vue'

const router = useRouter()
const store = useScriptEditorStore()

type TabKey = 'flow' | 'config' | 'characters' | 'assets' | 'validate'

const tabs: {
  key: TabKey
  label: string
  icon: 'adventure' | 'setting' | 'character' | 'background' | 'achievement'
}[] = [
  { key: 'flow', label: '章节流程', icon: 'adventure' },
  { key: 'config', label: '剧本设置', icon: 'setting' },
  { key: 'characters', label: '角色', icon: 'character' },
  { key: 'assets', label: '素材', icon: 'background' },
  { key: 'validate', label: '校验', icon: 'achievement' },
]

const assetKinds: { key: AssetKind; label: string }[] = [
  { key: 'background', label: '背景图' },
  { key: 'pic', label: '插图' },
  { key: 'music', label: '背景音乐' },
  { key: 'sound', label: '音效' },
  { key: 'ambient', label: '环境音' },
]

// ---- nav 指示条（与 SettingsNav 同一套做法）----
const navEl = ref<HTMLElement | null>(null)
const indicatorEl = ref<HTMLElement | null>(null)
const tabRefs: Record<string, HTMLElement | null> = {}

const setTabRef = (key: string, el: unknown) => {
  tabRefs[key] = (el as { $el?: HTMLElement } | null)?.$el ?? null
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

// 只由 watch 驱动，switchTab 里不重复调
watch(() => store.tab, moveIndicator)
watch(() => store.detail?.package.key, moveIndicator)

const switchTab = (key: TabKey) => {
  if (!store.detail && key !== 'flow') return
  store.tab = key
  if (key === 'validate') void store.runValidation()
  if (key === 'assets') void store.refreshGlobalAssets()
}

// ---- 面包屑 ----
const saveLabel = computed(() => {
  if (store.saving) return '正在保存…'
  if (store.dirty) return '有未保存改动'
  if (store.lastSavedAt) {
    const d = new Date(store.lastSavedAt)
    return `已自动保存 · ${String(d.getHours()).padStart(2, '0')}:${String(
      d.getMinutes(),
    ).padStart(2, '0')}`
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
const modal = ref<'script' | 'chapter' | 'character' | null>(null)

const modalTitle = computed(() =>
  modal.value === 'script' ? '新建剧本' : modal.value === 'chapter' ? '新建章节' : '新建角色',
)

const scriptForm = reactive({
  folderName: '',
  description: '',
  isAdventure: false,
  boundCharacterFolder: '',
})
const chapterForm = reactive({ id: '', name: '' })
const charForm = reactive({ folder: '', aiName: '', systemPrompt: '' })

const confirmModal = async () => {
  const which = modal.value
  modal.value = null
  if (which === 'script') {
    try {
      const pkg = await createScript({ ...scriptForm })
      Object.assign(scriptForm, {
        folderName: '',
        description: '',
        isAdventure: false,
        boundCharacterFolder: '',
      })
      await store.refreshScripts()
      await store.openScript(pkg.key)
    } catch (e) {
      store.notifyError('新建剧本失败', e)
    }
  } else if (which === 'chapter') {
    await store.createChapter(chapterForm.id, chapterForm.name)
    chapterForm.id = ''
    chapterForm.name = ''
  } else if (which === 'character') {
    await store.createCharacter(charForm.folder, charForm.aiName, charForm.systemPrompt)
    Object.assign(charForm, { folder: '', aiName: '', systemPrompt: '' })
  }
}

// ---- 其它动作 ----
const onRename = (e: Event) => store.setChapterName((e.target as HTMLInputElement).value)

const openFolder = async () => {
  if (!store.scriptKey) return
  try {
    await openScriptFolder(store.scriptKey)
  } catch (err) {
    store.notifyError('打开目录失败', err)
  }
}

const previewFrom = ref<string | undefined>(undefined)

const playtest = async () => {
  previewFrom.value = store.level === 'chapter' ? store.chapter?.id : undefined
  await store.startPreview(previewFrom.value)
}

const leave = async () => {
  await store.stopPreview()
  await store.flushPendingSave()
  // 先落盘再同步，顺序不能反：引擎重扫的是磁盘，没写完就同步等于同步了旧内容
  await store.syncEngine()
  void router.push('/')
}

// ---- 快捷键 ----
const onKey = (e: KeyboardEvent) => {
  const mod = e.ctrlKey || e.metaKey
  if (!mod) return
  const k = e.key.toLowerCase()

  // 在输入框里 Ctrl+Z 应该走浏览器原生撤销，否则作者想撤销一个词
  // 却把整个事件列表回退了一帧，而且刚敲的字（还没 change 提交）一起消失。
  const t = e.target as HTMLElement | null
  const inEditor =
    !!t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)

  if (k === 's') {
    e.preventDefault()
    void store.save()
    return
  }
  if (inEditor) return

  if (k === 'z' && !e.shiftKey) {
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
  void store.stopPreview()
  void store.flushPendingSave()
})
</script>

<style scoped>
.editor-root {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

/* 独立路由必须自带背景，窗口是 transparent 的 */
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

/* ---- 顶栏 ---- */
.snav {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: space-between;
  padding: 0.5rem 1.25rem;
}
.logo {
  margin-left: 1.25rem;
  font-size: 0.95rem;
  font-weight: 700;
  letter-spacing: 0.5px;
  color: var(--accent-color);
  white-space: nowrap;
}
.snav nav {
  position: relative;
  display: flex;
  height: 100%;
  width: 100%;
  flex-wrap: nowrap;
  align-items: center;
  justify-content: center;
  gap: 0.25rem;
  overflow-x: auto;
  overflow-y: hidden;
  padding: 0 0.5rem;
}
.indicator {
  position: absolute;
  bottom: 0;
  left: 0;
  z-index: 10;
  height: 0.25rem;
  width: 0;
  border-radius: 0.25rem;
  background: var(--accent-color);
  box-shadow: 0 0 10px rgba(121, 217, 255, 0.4);
}
.nav-badge {
  margin-left: 4px;
  border-radius: 99px;
  padding: 0 5px;
  font-size: 0.6rem;
  color: #fff;
  background: #ef4444;
}
.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 9999px;
  padding: 0.375rem;
  color: #fff;
  cursor: pointer;
  transition: all 0.3s ease-in-out;
}
.close-btn:hover {
  color: var(--accent-color);
  background: rgba(255, 255, 255, 0.1);
  transform: rotate(90deg);
}

/* ---- 面包屑 ---- */
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
.crumb .link {
  color: var(--accent-color);
}
.crumb .link:hover {
  text-decoration: underline;
}
.crumb .sep {
  opacity: 0.4;
}
.crumb .dim {
  font-size: 0.72rem;
  opacity: 0.35;
}
.crumb .right {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-left: auto;
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
.save-state i.clean {
  background: #4ade80;
}
.save-state i.pending {
  background: #fcd34d;
}

/* ---- 主体 ---- */
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
.col-main {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
}
.col-side {
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

.toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
}
.empty {
  padding: 2rem 0;
  text-align: center;
  font-size: 0.85rem;
  color: rgba(255, 255, 255, 0.45);
}
.notice {
  margin-bottom: 0.9rem;
  border-radius: 0.75rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(0, 0, 0, 0.16);
  padding: 0.7rem 0.85rem;
  font-size: 0.76rem;
  line-height: 1.85;
  color: rgba(255, 255, 255, 0.6);
}
.notice b {
  color: rgba(255, 255, 255, 0.85);
}
.notice code,
.f-hint code,
.ref {
  font-family: ui-monospace, Menlo, monospace;
  color: var(--accent-color);
}

/* ---- 卡片 ---- */
.script-card,
.row-card {
  width: 100%;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  padding: 11px 13px;
  margin-bottom: 8px;
  background: rgba(255, 255, 255, 0.06);
  transition: all 0.2s;
}
.script-card {
  cursor: pointer;
}
.script-card:hover {
  border-color: var(--accent-color);
  background: rgba(121, 217, 255, 0.08);
}
/* 与 ChapterFlow 的删除按钮同样的处理：默认隐形，hover 卡片才出现，
   免得一列叉号看着像随手就能点 */
.card-del {
  border-radius: 4px;
  padding: 0 5px;
  font-size: 11px;
  line-height: 1.4;
  color: rgba(255, 255, 255, 0.25);
  opacity: 0;
  transition: all 0.15s;
}
.script-card:hover .card-del {
  opacity: 1;
}
.card-del:hover {
  color: #fca5a5;
  background: rgba(248, 113, 113, 0.15);
}
.tag {
  border: 1px solid rgba(121, 217, 255, 0.35);
  border-radius: 99px;
  padding: 0 7px;
  font-size: 0.62rem;
  color: var(--accent-color);
  background: rgba(121, 217, 255, 0.12);
}
.tag.warn {
  color: #fcd34d;
  border-color: rgba(251, 191, 36, 0.35);
  background: rgba(251, 191, 36, 0.12);
}

/* ---- 表单 ---- */
.field {
  margin-bottom: 1rem;
}
.f-label {
  display: inline-flex;
  align-items: center;
  font-weight: 500;
  color: var(--accent-color);
  font-size: 0.9rem;
}
.f-label .req {
  margin-left: 2px;
  font-size: 0.7rem;
  color: #f87171;
}
.f-key {
  margin: 0.25rem 0 0.5rem;
  font-size: 0.8rem;
  color: #d1d5db;
}
.f-hint {
  margin-top: 0.3rem;
  font-size: 0.72rem;
  line-height: 1.7;
  color: rgba(255, 255, 255, 0.4);
}
.section {
  margin: 1rem 0;
  border-radius: 0.75rem;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(0, 0, 0, 0.15);
  padding: 1rem;
}
.inline-toggle {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.8rem;
  white-space: nowrap;
  color: rgba(255, 255, 255, 0.7);
}

/* ---- 素材 ---- */
.asset-group {
  margin-bottom: 1.1rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  padding-bottom: 0.9rem;
}
.asset-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.6rem;
}
.asset-title {
  font-size: 0.85rem;
  font-weight: 600;
  color: #fff;
}
.asset-head .chip {
  margin-left: 0;
}
.asset-head .chip:first-of-type {
  margin-left: auto;
}
.asset-cols {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}
.asset-sub {
  margin-bottom: 0.35rem;
  font-size: 0.7rem;
  color: rgba(255, 255, 255, 0.4);
}
.asset-empty {
  font-size: 0.72rem;
  color: rgba(255, 255, 255, 0.25);
}
.asset-item {
  display: inline-block;
  margin: 0 4px 4px 0;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 5px;
  padding: 1px 7px;
  font-size: 0.7rem;
  color: rgba(255, 255, 255, 0.7);
  background: rgba(255, 255, 255, 0.05);
}
.asset-item.global {
  border-color: rgba(167, 139, 250, 0.3);
  color: #c4b5fd;
  background: rgba(167, 139, 250, 0.1);
}

/* ---- 校验页 ---- */
.counts {
  font-size: 0.78rem;
  color: rgba(255, 255, 255, 0.5);
}
.counts b,
.diag-counts b {
  font-weight: 600;
}
.err {
  color: #fca5a5;
}
.warn {
  color: #fcd34d;
}
.info {
  color: rgba(255, 255, 255, 0.5);
}
.ok-banner {
  border-radius: 0.75rem;
  border: 1px solid rgba(74, 222, 128, 0.3);
  background: rgba(74, 222, 128, 0.1);
  padding: 0.9rem;
  font-size: 0.82rem;
  color: #86efac;
}
.diag-group {
  margin-bottom: 0.75rem;
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(0, 0, 0, 0.15);
  overflow: hidden;
}
.diag-group.clean {
  opacity: 0.55;
}
.diag-head {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  padding: 0.55rem 0.8rem;
}
.diag-title {
  font-size: 0.82rem;
  font-weight: 600;
  color: #fff;
}
.diag-file {
  font-family: ui-monospace, Menlo, monospace;
  font-size: 0.66rem;
  color: rgba(255, 255, 255, 0.3);
}
.diag-counts {
  display: flex;
  gap: 0.6rem;
  margin-left: auto;
  font-size: 0.7rem;
}
.pass {
  color: #86efac;
}
.diag {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  padding: 0.45rem 0.8rem;
  font-size: 0.76rem;
  line-height: 1.75;
  color: rgba(255, 255, 255, 0.75);
}
.diag.clickable {
  cursor: pointer;
}
.diag.clickable:hover {
  background: rgba(255, 255, 255, 0.05);
}
.diag .dot {
  flex: 0 0 auto;
  width: 6px;
  height: 6px;
  margin-top: 0.55rem;
  border-radius: 50%;
}
.diag.error .dot {
  background: #f87171;
}
.diag.warn .dot {
  background: #fbbf24;
}
.diag.info .dot {
  background: rgba(255, 255, 255, 0.3);
}
.diag .msg {
  flex: 1;
}
.diag .loc {
  flex: 0 0 auto;
  font-size: 0.68rem;
  white-space: nowrap;
  color: var(--accent-color);
  opacity: 0.7;
}

/* ---- chip ---- */
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
.chip.primary {
  border-color: rgba(121, 217, 255, 0.45);
  color: var(--accent-color);
  background: rgba(121, 217, 255, 0.14);
}
.chip.primary:hover {
  background: rgba(121, 217, 255, 0.24);
}

/* ---- 弹窗 ---- */
.modal-mask {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  backdrop-filter: blur(5px);
  background: rgba(0, 0, 0, 0.45);
}
.modal {
  width: min(440px, 92vw);
  max-height: 86vh;
  overflow-y: auto;
  border: 1px solid rgba(255, 255, 255, 0.125);
  border-radius: 12px;
  padding: 15px;
  background: rgba(30, 41, 59, 0.97);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
}
.modal-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  border-bottom: 2px solid var(--accent-color);
  padding-bottom: 0.5rem;
  margin-bottom: 1rem;
}
.modal-head h4 {
  font-weight: 600;
  color: #fff;
}
.modal-x {
  margin-left: auto;
  color: rgba(255, 255, 255, 0.5);
  transition: all 0.3s;
}
.modal-x:hover {
  color: var(--accent-color);
  transform: rotate(90deg);
}
.modal-foot {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1.25rem;
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
</style>
