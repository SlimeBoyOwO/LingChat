<template>
  <!--
    背景层必须自己造。窗口是 transparent: true（tauri.conf.json），设置面板之所以
    能透出画面是因为它盖在 MainChat 上；/script-editor 是独立路由，底下什么都没有，
    不给背景就直接透出桌面。Credits.vue 同理显式加了 bg-[#0a0a0c]。
    这里用渐变而不是背景图，避免依赖 Git LFS 资源。
  -->
  <div class="editor-root relative w-full h-full overflow-hidden">
    <div class="bg-layer"></div>

    <!-- 顶栏：与 SettingsNav 同构 -->
    <div class="flex w-full items-center justify-between px-5 py-2">
      <span class="ml-5 text-[0.95rem] font-bold tracking-[0.5px] text-brand whitespace-nowrap">LingChat 剧本编辑器</span>
      <nav
        ref="navEl"
        class="relative flex h-full w-full flex-nowrap items-center justify-center gap-1 overflow-x-auto overflow-y-hidden px-2"
      >
        <div
          ref="indicatorEl"
          class="absolute bottom-0 left-0 z-10 h-1 w-0 rounded-sm bg-brand shadow-[0_0_10px_rgba(121,217,255,0.4)]"
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
            class="ml-1 rounded-full px-[5px] text-[0.6rem] text-white bg-red-500"
            >{{ store.report.errorCount }}</span
          >
        </Button>
      </nav>
      <Icon
        icon="close"
        :size="40"
        class="flex items-center justify-center rounded-full p-1.5 text-white cursor-pointer transition-all duration-300 ease-in-out hover:text-brand hover:bg-white/10 hover:rotate-90"
        @click="leave"
      />
    </div>

    <!-- 面包屑 -->
    <div class="flex items-center gap-2 px-8 pb-1 text-[0.8rem] text-white/55">
      <button
        v-if="store.detail"
        class="text-brand hover:underline"
        @click="store.closeScript()"
      >
        ‹ 剧本列表
      </button>
      <span v-else>首页</span>

      <template v-if="store.detail">
        <span class="opacity-40">›</span>
        <button
          v-if="store.level === 'chapter'"
          class="text-brand hover:underline"
          @click="store.backToFlow()"
        >
          {{ store.detail.package.scriptName }}
        </button>
        <b
          v-else
          class="font-semibold text-white"
          >{{ store.detail.package.scriptName }}</b
        >

        <template v-if="store.level === 'chapter' && store.chapter">
          <span class="opacity-40">›</span>
          <b class="font-semibold text-white">{{ store.chapter.name || store.chapter.id }}</b>
          <span class="text-[0.72rem] opacity-35">{{ store.chapter.id }}.yaml</span>
        </template>
      </template>

      <span class="flex items-center gap-3 ml-auto">
        <span
          v-if="store.detail"
          class="inline-flex items-center gap-[5px] text-[0.75rem] text-white/50"
        >
          <i
            class="inline-block w-1.5 h-1.5 rounded-full"
            :class="store.dirty ? 'bg-amber-300' : 'bg-green-400'"
          ></i>
          {{ saveLabel }}
        </span>
        <template v-if="store.level === 'chapter'">
          <button
            class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="!store.canUndo"
            title="撤销（Ctrl / ⌘ + Z）"
            @click="store.undo()"
          >
            ↩ 撤销
          </button>
          <button
            class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
            :disabled="!store.canRedo"
            title="重做（Ctrl / ⌘ + Shift + Z）"
            @click="store.redo()"
          >
            ↪ 重做
          </button>
        </template>
        <button
          class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
          title="查看全部快捷键（? 键）"
          @click="shortcutHelp = true"
        >
          快捷键
        </button>
        <template v-if="store.detail">
          <button
            class="inline-flex items-center gap-1 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap transition-all duration-200 border border-brand/45 text-brand bg-brand/14 hover:bg-brand/24"
            title="Ctrl / ⌘ + Enter"
            @click="playtest"
          >
            {{ store.level === 'chapter' ? '从本章试玩' : '从开场试玩' }}
          </button>
        </template>
      </span>
    </div>

    <!-- 试玩前置条件不满足时的常驻提示。等作者点了「试玩」才报，他会先对着
         一个卡住不动的画面困惑一阵 —— 那正是这条横幅要省掉的时间。 -->
    <div
      v-if="store.detail && store.readiness && !store.readiness.ok"
      class="flex items-center gap-2.5 mx-5 mb-2 border border-amber-300/30 rounded-lg px-3 py-[7px] text-[0.76rem] leading-[1.7] text-amber-100/90 bg-amber-300/10"
    >
      <span class="shrink-0 border border-amber-300/40 rounded-full px-2 py-px text-[0.66rem] font-semibold text-amber-300">试玩会卡住</span>
      <span>{{ store.readiness.reason }}</span>
    </div>

    <!-- 主体 -->
    <div class="flex h-[calc(100%-5.5rem)] min-h-0 flex-col">
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
            class="py-8 text-center text-[0.85rem] text-white/45"
          >
            正在读取…
          </p>
          <p
            v-else-if="store.scripts.length === 0"
            class="py-8 text-center text-[0.85rem] text-white/45"
          >
            还没有任何剧本，点下面新建一个
          </p>

          <div
            v-for="s in store.scripts"
            :key="s.key"
            class="w-full border border-white/10 rounded-[10px] px-[13px] py-[11px] mb-2 bg-white/6 transition-all duration-200 cursor-pointer hover:border-brand hover:bg-[rgba(121,217,255,0.08)] group"
            @click="store.openScript(s.key)"
          >
            <div class="flex items-baseline gap-2">
              <span class="font-semibold text-white">{{ s.scriptName }}</span>
              <span
                v-if="s.isAdventure"
                class="border border-brand/35 rounded-full px-[7px] text-[0.62rem] text-brand bg-brand/12"
                >羁绊冒险</span
              >
              <span
                v-if="!s.loadedByEngine"
                class="border border-amber-300/35 rounded-full px-[7px] text-[0.62rem] text-amber-300 bg-amber-300/12"
                >未加载</span
              >
              <span class="ml-auto text-xs text-white/40">{{ s.chapterCount }} 章</span>
              <button
                class="rounded px-[5px] text-[11px] leading-[1.4] text-white/25 opacity-0 transition-all duration-150 group-hover:opacity-100 hover:text-red-300 hover:bg-red-400/15"
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
          <div class="flex flex-wrap items-center gap-2 mb-3">
            <button
              class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
              @click="modal = 'chapter'"
            >
              ＋ 新建章节
            </button>
            <button
              class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
              @click="store.runValidation()"
            >
              重新校验
            </button>
            <button
              class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
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
        class="flex w-[94%] min-h-0 flex-1 gap-5 mx-auto px-3 py-4"
      >
        <div class="flex min-w-0 flex-1 flex-col">
          <MenuItem
            title="事件时间线"
            class="fill flex h-full min-h-0 flex-col"
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
              <label
                class="inline-flex items-center gap-2 text-[0.8rem] whitespace-nowrap text-white/70"
                :title="FOLD_HINT"
              >
                <Toggle
                  :checked="store.foldCompounds"
                  @change="(v: boolean) => (store.foldCompounds = v)"
                />
                合并转场等固定组合
              </label>
              <span class="shrink-0 text-xs text-white/40">
                {{ store.chapter?.events.length ?? 0 }} 个事件
              </span>
            </div>
            <div class="min-h-0 flex-1 overflow-y-auto pr-1">
              <ChapterTimeline />
            </div>
          </MenuItem>
        </div>

        <div class="flex min-h-0 flex-[0_0_340px] flex-col">
          <MenuItem
            title="事件属性"
            class="fill flex h-full min-h-0 flex-col"
          >
            <template #header>
              <Icon
                icon="setting"
                :size="20"
              />
            </template>
            <div class="min-h-0 flex-1 overflow-y-auto pr-1">
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

          <p class="mb-[0.9rem] rounded-xl border border-white/10 bg-black/16 px-[0.85rem] py-[0.7rem] text-[0.76rem] leading-[1.85] text-white/60">
            改写 <code class="font-mono text-brand">story_config.yaml</code> 会丢掉文件里的 YAML 注释（六个官方剧本的
            config 都带中文注释）。保存前会自动留一份 <code class="font-mono text-brand">.bak</code>。
          </p>

          <div
            v-for="f in store.schema?.storyConfigFields ?? []"
            :key="f.key"
            class="mb-4"
          >
            <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">
              {{ f.label }}<span
                v-if="f.required"
                class="ml-0.5 text-[0.7rem] text-red-400"
                >＊</span
              >
            </label>
            <p class="my-1 mb-2 text-[0.8rem] text-gray-300">{{ f.key }}</p>
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
              class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40 [&_code]:font-mono [&_code]:text-brand"
            >
              {{ f.hint }}
            </p>
          </div>

          <!-- 羁绊冒险 -->
          <div class="my-4 rounded-xl border border-white/10 bg-black/15 p-4">
            <label class="inline-flex items-center gap-2 text-[0.8rem] whitespace-nowrap text-white/70 mb-2">
              <Toggle
                :checked="isAdventure"
                @change="toggleAdventure"
              />
              这是某个角色的羁绊冒险
            </label>
            <template v-if="isAdventure">
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">绑定角色目录名</label>
                <p class="my-1 mb-2 text-[0.8rem] text-gray-300">adventure.bound_character_folder</p>
                <input
                  class="glass-input"
                  :value="adventureField('bound_character_folder')"
                  @change="(e) => onAdventureText('bound_character_folder', e)"
                />
              </div>
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">排序</label>
                <p class="my-1 mb-2 text-[0.8rem] text-gray-300">adventure.order</p>
                <input
                  class="glass-input"
                  type="number"
                  :value="adventureField('order')"
                  @change="(e) => onAdventureNumber('order', e)"
                />
                <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40">数值越小越靠前显示，决定羁绊冒险在角色卡上的排列顺序</p>
              </div>
              <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40 [&_code]:font-mono [&_code]:text-brand">
                解锁条件（<code class="font-mono text-brand">unlock_conditions</code>）目前保持文件里的原值不动，
                下一轮补可视化编辑。<code class="font-mono text-brand">trigger.mode</code> 引擎没有任何消费者，
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

          <p class="mb-[0.9rem] rounded-xl border border-white/10 bg-black/16 px-[0.85rem] py-[0.7rem] text-[0.76rem] leading-[1.85] text-white/60">
            剧本里用 <code class="font-mono text-brand">character: &lt;下面的引用名&gt;</code> 指代角色；写
            <code class="font-mono text-brand">MAIN</code> 表示当前主角（羁绊剧本里就是绑定的那位）。
            <b class="font-semibold text-white/85">引擎只在本剧本的 <code class="font-mono text-brand">characters/</code> 里找人</b>，所以想用全局角色库里
            已有的人设，得先「导入」一份到这里 —— 导入复制的是人设文件，立绘仍读全局那份，
            不会让剧本目录白白变大。
            <br /><span class="font-semibold text-amber-300">提示：</span>羁绊剧本里 <code class="font-mono text-brand">character: MAIN</code> 绑定的角色
            <b class="font-semibold text-white/85">不需要导入</b>，引擎会直接从全局角色库读取它的人设和立绘。
          </p>

          <p
            v-if="store.characters.length === 0"
            class="py-8 text-center text-[0.85rem] text-white/45"
          >
            还没有剧本内角色
          </p>
          <div
            v-for="c in store.characters"
            :key="c.folder"
            class="w-full border border-white/10 rounded-[10px] px-[13px] py-[11px] mb-2 bg-white/6 transition-all duration-200 flex items-center group"
          >
            <!-- 立绘缩略图：本地 avatar 优先，没有回退全局；都没有时占位，与
                 引擎运行时同一个查找顺序，避免「编辑器看着有、游戏里没有」 -->
            <div class="char-thumb shrink-0 w-11 h-11 rounded-full overflow-hidden border-[1.5px] border-brand/35">
              <img
                v-if="c.previewImage"
                :src="assetUrl(c.previewImage)"
                :alt="c.aiName"
                class="w-full h-full object-cover object-[top_center]"
                loading="lazy"
              />
              <span
                v-else
                class="flex items-center justify-center w-full h-full text-[0.56rem] text-white/35"
                >无立绘</span>
            </div>
            <div class="flex min-w-0 flex-1 flex-col gap-0.5">
              <div class="flex items-baseline gap-2">
                <span class="font-semibold text-white">{{ c.aiName }}</span>
                <code class="font-mono text-brand">character: {{ c.roleKey }}</code>
                <span
                  v-if="c.emotions.length === 0 && c.globalAvatar"
                  class="shrink-0 border border-brand/40 rounded-full px-[7px] py-px text-[0.6rem] text-brand bg-brand/12"
                  title="本剧本没复制立绘，但全局角色库里有；引擎会自动用全局那份"
                  >立绘读自全局</span
                >
                <span class="ml-auto text-xs text-white/40">
                  {{ c.emotions.length }} 个表情{{
                    c.clothes.length ? ` · ${c.clothes.length} 套服装` : ''
                  }}
                </span>
              </div>
              <p
                v-if="!c.previewImage"
                class="mt-1 text-xs text-yellow-200"
              >
                本剧本与全局角色库都没有这个角色的立绘，台词里它不会显示
              </p>
              <p
                v-else
                class="mt-1 text-xs text-white/40"
              >
                {{ c.emotions.slice(0, 12).join('、') }}{{ c.emotions.length > 12 ? ' …' : '' }}
              </p>
            </div>
            <button
              class="shrink-0 rounded px-[5px] text-[11px] text-white/25 opacity-0 transition-all duration-150 group-hover:opacity-100 hover:text-red-300 hover:bg-red-400/15"
              title="删除角色（移到 .trash/）"
              @click="store.deleteCharacter(c.folder, c.aiName)"
            >
              ✕
            </button>
          </div>

          <div class="mt-4 flex flex-wrap gap-2">
            <Button
              type="big"
              @click="modal = 'character'"
            >
              ＋ 新建角色
            </Button>
            <Button
              type="big"
              @click="modal = 'importChar'"
            >
              ↓ 从全局角色库导入
            </Button>
          </div>
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

          <p class="mb-[0.9rem] rounded-xl border border-white/10 bg-black/16 px-[0.85rem] py-[0.7rem] text-[0.76rem] leading-[1.85] text-white/60">
            引擎查找素材的顺序是<b class="font-semibold text-white/85">先本剧本，再全局</b>，所以两处都能被找到，区别在于：
            <b class="font-semibold text-white/85">剧本素材</b>随剧本一起分发，别的剧本看不到；<b class="font-semibold text-white/85">全局素材</b>所有剧本共享，
            但导出剧本时不会带走。
          </p>

          <div
            v-for="k in assetKinds"
            :key="k.key"
            class="mb-[1.1rem] border-b border-white/[0.07] pb-[0.9rem]"
          >
            <div class="flex items-center gap-2 mb-[0.6rem]">
              <span class="text-[0.85rem] font-semibold text-white">{{ k.label }}</span>
              <button
                class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40 ml-auto"
                @click="importAsset(k.key, 'script')"
              >
                导入到本剧本
              </button>
              <button
                v-if="k.key !== 'sound'"
                class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
                @click="importAsset(k.key, 'global')"
              >
                导入为全局
              </button>
              <span
                v-if="k.key === 'sound'"
                class="ml-auto text-[0.66rem] text-white/40"
                >音效只属于本剧本，没有全局目录</span
              >
            </div>
            <div class="grid grid-cols-2 gap-4 has-[>:only-child]:grid-cols-1">
              <div
                v-for="col in scopesFor(k.key)"
                :key="col"
              >
                <p class="mb-[0.35rem] text-[0.7rem] text-white/40">
                  {{ col === 'script' ? '本剧本' : '全局' }} ·
                  {{ filesOf(col, k.key).length }}
                </p>
                <p
                  v-if="filesOf(col, k.key).length === 0"
                  class="text-[0.72rem] text-white/25"
                >
                  无
                </p>
                <div
                  v-for="f in filesOf(col, k.key)"
                  :key="f.path"
                  class="relative flex items-center gap-[9px] mb-1.5 border border-white/10 rounded-lg px-[9px] py-[7px] bg-white/4 transition-all duration-150 hover:border-white/[0.22] hover:bg-white/7 group"
                  :class="{ 'border-purple-400/[0.22] bg-purple-400/7': col === 'global' }"
                >
                  <!-- 图片直接出缩略图；音频给一个原生播放器，够用且零依赖 -->
                  <img
                    v-if="isImageKind(k.key)"
                    class="asset-thumb shrink-0 w-14 h-10 rounded-[5px] object-cover"
                    :src="assetUrl(f.path)"
                    :alt="f.name"
                    loading="lazy"
                  />
                  <div class="flex min-w-0 flex-1 flex-col gap-[3px]">
                    <span class="overflow-hidden text-[0.74rem] text-ellipsis whitespace-nowrap text-white/80">{{ f.name }}</span>
                    <span class="text-[0.64rem] text-white/35">{{ humanSize(f.size) }}</span>
                    <audio
                      v-if="!isImageKind(k.key)"
                      class="w-full h-[26px]"
                      controls
                      preload="none"
                      controlslist="nodownload noremoteplayback"
                      :src="assetUrl(f.path)"
                    ></audio>
                  </div>
                  <button
                    class="shrink-0 rounded px-[5px] text-[11px] text-white/25 opacity-0 transition-all duration-150 group-hover:opacity-100 hover:text-red-300 hover:bg-red-400/15"
                    title="删除（移到 .trash/）"
                    @click="store.deleteAsset(k.key, col, f.name)"
                  >
                    ✕
                  </button>
                </div>
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

          <div class="flex flex-wrap items-center gap-2 mb-3">
            <button
              class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
              @click="store.runValidation()"
            >
              重新校验
            </button>
            <span
              v-if="store.report"
              class="text-[0.78rem] text-white/50 [&_b]:font-semibold"
            >
              <b class="text-red-300">{{ store.report.errorCount }}</b> 错误 ·
              <b class="text-amber-300">{{ store.report.warnCount }}</b> 警告 ·
              <b class="text-white/50">{{ store.report.infoCount }}</b> 提示
            </span>
          </div>

          <p
            v-if="!store.report"
            class="py-8 text-center text-[0.85rem] text-white/45"
          >
            正在校验…
          </p>
          <p
            v-else-if="store.report.diagnostics.length === 0"
            class="rounded-xl border border-green-400/30 bg-green-400/10 px-[0.9rem] py-[0.9rem] text-[0.82rem] text-green-300"
          >
            没有发现问题，这个剧本可以正常跑起来。
          </p>

          <template v-else>
            <!-- 剧本级问题 -->
            <div
              v-if="store.scriptDiagnostics.length"
              class="mb-3 rounded-[10px] border border-white/10 bg-black/15 overflow-hidden"
            >
              <div class="flex items-center gap-[0.6rem] border-b border-white/[0.07] px-[0.8rem] py-[0.55rem]">
                <span class="text-[0.82rem] font-semibold text-white">剧本整体</span>
                <span class="font-mono text-[0.66rem] text-white/30">story_config.yaml</span>
              </div>
              <div
                v-for="(d, i) in store.scriptDiagnostics"
                :key="i"
                class="flex items-start gap-2 px-[0.8rem] py-[0.45rem] text-[0.76rem] leading-[1.75] text-white/75"
              >
                <span
                  class="shrink-0 w-1.5 h-1.5 mt-[0.55rem] rounded-full"
                  :class="{ 'bg-red-400': d.severity === 'error', 'bg-amber-400': d.severity === 'warn', 'bg-white/30': d.severity === 'info' }"
                ></span>
                <span class="flex-1">{{ d.message }}</span>
              </div>
            </div>

            <!-- 按章节聚合，与流程图同样的顺序 -->
            <div
              v-for="c in store.chapters"
              :key="c.id"
              class="mb-3 rounded-[10px] border border-white/10 bg-black/15 overflow-hidden"
              :class="{ 'opacity-55': !chapterHas(c.id) }"
            >
              <div class="flex items-center gap-[0.6rem] border-b border-white/[0.07] px-[0.8rem] py-[0.55rem]">
                <span class="text-[0.82rem] font-semibold text-white">{{ c.name || c.id }}</span>
                <span class="font-mono text-[0.66rem] text-white/30">{{ c.id }}.yaml</span>
                <span class="flex gap-[0.6rem] ml-auto text-[0.7rem] [&_b]:font-semibold">
                  <b
                    v-if="store.diagnosticsByChapter[c.id]?.errors"
                    class="text-red-300"
                    >{{ store.diagnosticsByChapter[c.id].errors }} 错误</b
                  >
                  <b
                    v-if="store.diagnosticsByChapter[c.id]?.warns"
                    class="text-amber-300"
                    >{{ store.diagnosticsByChapter[c.id].warns }} 警告</b
                  >
                  <b
                    v-if="store.diagnosticsByChapter[c.id]?.infos"
                    class="text-white/50"
                    >{{ store.diagnosticsByChapter[c.id].infos }} 提示</b
                  >
                  <span
                    v-if="!chapterHas(c.id)"
                    class="text-green-300"
                    >通过</span
                  >
                </span>
                <button
                  class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
                  @click="store.openChapter(c.id)"
                >
                  打开
                </button>
              </div>

              <div
                v-for="(d, i) in diagnosticsOf(c.id)"
                :key="i"
                class="flex items-start gap-2 px-[0.8rem] py-[0.45rem] text-[0.76rem] leading-[1.75] text-white/75 cursor-pointer hover:bg-white/5"
                @click="jumpTo(d)"
              >
                <span
                  class="shrink-0 w-1.5 h-1.5 mt-[0.55rem] rounded-full"
                  :class="{ 'bg-red-400': d.severity === 'error', 'bg-amber-400': d.severity === 'warn', 'bg-white/30': d.severity === 'info' }"
                ></span>
                <span class="flex-1">{{ d.message }}</span>
                <span
                  v-if="d.eventIndex !== undefined"
                  class="shrink-0 text-[0.68rem] whitespace-nowrap text-brand opacity-70"
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
      <Transition
        enter-active-class="transition-opacity duration-200 ease"
        leave-active-class="transition-opacity duration-200 ease"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="modal"
          class="fixed inset-0 z-[9999] flex items-center justify-center p-4 backdrop-blur-md bg-black/55"
          @click.self="modal = null"
        >
          <div class="w-[min(440px,92vw)] max-h-[86vh] overflow-y-auto border border-white/12.5 rounded-xl py-4 px-[18px] pb-[18px] bg-[rgba(12,20,30,0.86)] backdrop-blur-lg backdrop-saturate-[1.4] shadow-[0_8px_32px_rgba(0,0,0,0.45),inset_0_1px_1px_rgba(255,255,255,0.06)]">
            <div class="flex items-center gap-2 border-b-2 border-brand pb-2 mb-4">
              <h4 class="font-semibold text-white">{{ modalTitle }}</h4>
              <button
                class="ml-auto text-white/50 transition-all duration-300 hover:text-brand hover:rotate-90"
                @click="modal = null"
              >
                ✕
              </button>
            </div>

            <template v-if="modal === 'script'">
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">剧本名</label>
                <input
                  v-model="scriptForm.folderName"
                  class="glass-input"
                  placeholder="例如：一起看星星"
                />
                <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40">同时作为目录名。羁绊冒险用目录名作全局主键，不能重名。</p>
              </div>
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">简介</label>
                <textarea
                  v-model="scriptForm.description"
                  class="glass-input min-h-16"
                ></textarea>
              </div>
              <label class="inline-flex items-center gap-2 text-[0.8rem] whitespace-nowrap text-white/70">
                <Toggle
                  :checked="scriptForm.isAdventure"
                  @change="(v: boolean) => (scriptForm.isAdventure = v)"
                />
                这是某个角色的羁绊冒险
              </label>
              <div
                v-if="scriptForm.isAdventure"
                class="mb-4 mt-2"
              >
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">绑定角色的目录名</label>
                <input
                  v-model="scriptForm.boundCharacterFolder"
                  class="glass-input"
                  placeholder="game_data/characters/ 下的目录名"
                />
              </div>
            </template>

            <template v-else-if="modal === 'chapter'">
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">章节文件名</label>
                <input
                  v-model="chapterForm.id"
                  class="glass-input"
                  placeholder="main2，或 Intro/intro2 放进子目录"
                />
              </div>
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">显示名</label>
                <input
                  v-model="chapterForm.name"
                  class="glass-input"
                  placeholder="例如：2 樱花的公园"
                />
                <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40">新章节自带一条「章节结束」，免得一保存就报缺少结束事件。</p>
              </div>
            </template>

            <!-- 从全局角色库导入 -->
            <template v-else-if="modal === 'importChar'">
              <p
                v-if="store.globalCharacters.length === 0"
                class="py-8 text-center text-[0.85rem] text-white/45"
              >
                全局角色库（game_data/characters/）是空的
              </p>
              <div
                v-for="g in store.globalCharacters"
                :key="g.folder"
                :class="[
                  'flex items-baseline gap-2 mb-1.5 border rounded-lg px-[11px] py-[9px] bg-white/5 transition-all duration-150',
                  g.alreadyInScript
                    ? 'cursor-default border-white/10 opacity-45'
                    : 'cursor-pointer border-white/10 hover:border-brand hover:bg-[rgba(121,217,255,0.08)]',
                  importForm.folder === g.folder && !g.alreadyInScript ? '!border-brand bg-brand/20 ring-1 ring-brand/30' : '',
                ]"
                @click="g.alreadyInScript || (importForm.folder = g.folder)"
              >
                <span class="font-semibold text-white">{{ g.aiName }}</span>
                <code class="font-mono text-brand">{{ g.folder }}</code>
                <span
                  v-if="g.alreadyInScript"
                  class="ml-auto text-xs text-white/35"
                  >已在本剧本</span
                >
                <span
                  v-else-if="!g.hasAvatar"
                  class="ml-auto text-xs text-yellow-200"
                  >没有立绘</span
                >
              </div>

              <label class="inline-flex items-center gap-2 text-[0.8rem] whitespace-nowrap text-white/70 mt-3">
                <Toggle
                  :checked="importForm.withAvatar"
                  @change="(v: boolean) => (importForm.withAvatar = v)"
                />
                连立绘一起复制
              </label>
              <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40 [&_code]:font-mono [&_code]:text-brand">
                默认不复制：引擎找立绘时本来就先看
                <code class="font-mono text-brand">game_data/characters/&lt;同名目录&gt;/avatar</code>，会自动命中，
                复制一份只是让剧本目录变大。只有打算把剧本单独发给<b class="font-semibold text-white/85">没有这个角色</b>的人时，
                才需要勾上。
              </p>
            </template>

            <template v-else>
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">角色目录名</label>
                <input
                  v-model="charForm.folder"
                  class="glass-input"
                  placeholder="剧本里会写 character: 这个名字"
                />
              </div>
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">显示名</label>
                <input
                  v-model="charForm.aiName"
                  class="glass-input"
                />
              </div>
              <div class="mb-4">
                <label class="inline-flex items-center font-medium text-brand text-[0.9rem]">人物设定</label>
                <textarea
                  v-model="charForm.systemPrompt"
                  class="glass-input min-h-24"
                  placeholder="这个角色的性格、说话方式、与主角的关系…"
                ></textarea>
              </div>
              <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40 [&_code]:font-mono [&_code]:text-brand">
                创建后请把立绘放进 <code class="font-mono text-brand">characters/&lt;目录名&gt;/avatar/</code>，
                文件名用情绪名（如 <code class="font-mono text-brand">正常.png</code>）。
              </p>
            </template>

            <div class="flex justify-end gap-2 mt-5">
              <button
                class="inline-flex items-center gap-1 border border-white/10 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 bg-white/6 transition-all duration-200 hover:enabled:text-white hover:enabled:bg-white/[0.12] disabled:cursor-not-allowed disabled:opacity-40"
                @click="modal = null"
              >
                取消
              </button>
              <button
                class="inline-flex items-center gap-1 rounded-lg px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap transition-all duration-200 border border-brand/45 text-brand bg-brand/14 hover:bg-brand/24"
                @click="confirmModal"
              >
                确定
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- ============ 快捷键表 ============ -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-opacity duration-200 ease"
        leave-active-class="transition-opacity duration-200 ease"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="shortcutHelp"
          class="fixed inset-0 z-[9999] flex items-center justify-center p-4 backdrop-blur-md bg-black/55"
          @click.self="shortcutHelp = false"
        >
          <div class="w-[min(440px,92vw)] max-h-[86vh] overflow-y-auto border border-white/12.5 rounded-xl py-4 px-[18px] pb-[18px] bg-[rgba(12,20,30,0.86)] backdrop-blur-lg backdrop-saturate-[1.4] shadow-[0_8px_32px_rgba(0,0,0,0.45),inset_0_1px_1px_rgba(255,255,255,0.06)]">
            <div class="flex items-center gap-2 border-b-2 border-brand pb-2 mb-4">
              <h4 class="font-semibold text-white">快捷键</h4>
              <button
                class="ml-auto text-white/50 transition-all duration-300 hover:text-brand hover:rotate-90"
                @click="shortcutHelp = false"
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
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { Button, Icon, Toggle } from '@/components/base'
import { MenuPage, MenuItem } from '@/components/ui'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { useGameStore } from '@/stores/modules/game'
import { createScript, openScriptFolder } from '@/api/services/script-editor'
import type { AssetFile, AssetKind, AssetScope, Diagnostic } from '@/api/services/script-editor'
import ChapterFlow from '@/components/script-editor/ChapterFlow.vue'
import ChapterTimeline from '@/components/script-editor/ChapterTimeline.vue'
import EventPropertyPanel from '@/components/script-editor/EventPropertyPanel.vue'
import PreviewStage from '@/components/script-editor/PreviewStage.vue'

const router = useRouter()
const store = useScriptEditorStore()
const gameStore = useGameStore()

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

watch(() => store.tab, () => moveIndicator())
watch(() => store.detail?.package.key, () => moveIndicator())

let navObserver: ResizeObserver | null = null

const observeNav = () => {
  if (typeof ResizeObserver === 'undefined' || !navEl.value) return
  // 不加动画：这类重算是「布局变了跟着修正」，滑一下反而像在乱动
  navObserver = new ResizeObserver(() => void moveIndicator(false))
  navObserver.observe(navEl.value)
  for (const el of Object.values(tabRefs)) if (el) navObserver.observe(el)
}

const switchTab = (key: TabKey) => {
  if (!store.detail && key !== 'flow') return
  store.tab = key
  if (key === 'validate') void store.runValidation()
  if (key === 'assets') {
    void store.refreshGlobalAssets()
    void store.refreshAssetFiles()
  }
  if (key === 'characters') void store.refreshGlobalCharacters()
  // 回到流程图时强制走一遍「落盘 → 重新校验」，图上画的才是磁盘里的真状态
  if (key === 'flow' && store.level === 'flow') void store.backToFlow()
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
const modal = ref<'script' | 'chapter' | 'character' | 'importChar' | null>(null)

const importForm = reactive({ folder: '', withAvatar: false })

const MODAL_TITLES: Record<string, string> = {
  script: '新建剧本',
  chapter: '新建章节',
  character: '新建角色',
  importChar: '从全局角色库导入',
}
const modalTitle = computed(() => MODAL_TITLES[modal.value ?? ''] ?? '')

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
  } else if (which === 'importChar') {
    if (!importForm.folder) return
    await store.importGlobalCharacter(importForm.folder, importForm.withAvatar)
    importForm.folder = ''
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

// ---- 快捷键表。既驱动实现，也直接渲染成帮助面板，不会两处走偏 ----
const shortcutHelp = ref(false)

/** 抽成常量纯粹是因为 title 内联会超出 100 列的行宽 */
const FOLD_HINT =
  '官方剧本里反复出现两组固定写法：「角色退场 → 背景 → 角色出场」的转场，' +
  '和「AI 说 → 等玩家输入 → AI 说」的一轮互动。打开后它们各折成一行，' +
  '长章节能少掉近一半行数；折起来的那行会写明这段切到哪个背景、用的什么提示。'

const SHORTCUTS: { keys: string; desc: string }[] = [
  { keys: 'Ctrl / ⌘ + S', desc: '立刻保存（平时是改完自动存，这条是给不放心的人的）' },
  { keys: 'Ctrl / ⌘ + Z', desc: '撤销' },
  { keys: 'Ctrl / ⌘ + Shift + Z', desc: '重做（还原刚才撤销的操作，Ctrl+Y 也行）' },
  { keys: 'Ctrl / ⌘ + D', desc: '复制选中的事件' },
  { keys: 'Ctrl / ⌘ + Enter', desc: '从当前位置试玩' },
  { keys: 'Delete', desc: '删除选中的事件' },
  { keys: '↑ / ↓', desc: '在事件之间移动光标' },
  { keys: 'Alt + ↑ / ↓', desc: '把选中的事件上移 / 下移' },
  { keys: 'Esc', desc: '结束试玩 / 返回上一层' },
  { keys: '?', desc: '打开这张表' },
]

const leave = async () => {
  await store.stopPreview()
  await store.flushPendingSave()
  // 先落盘再同步，顺序不能反：引擎重扫的是磁盘，没写完就同步等于同步了旧内容
  await store.syncEngine()
  // 退出编辑器前标记 game 未初始化 —— MainChat 据此决定重跑 initializeGame。
  // 否则从编辑器回自由对话时 gameStore.initialized=true 已设，不会重新 init，
  // 编辑器里可能已污染的 presentRoleIds/mainRoleId/gameRoles 直接带入自由对话。
  gameStore.initialized = false
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
  await moveIndicator(false)
  observeNav()
})

onUnmounted(async () => {
  window.removeEventListener('keydown', onKey)
  navObserver?.disconnect()
  navObserver = null
  // 等待试玩完全停止（含后端恢复）再标记未初始化，避免 MainChat 随后立即
  // 挂载时读到后端尚未还原的脏 line_list。
  try {
    await store.stopPreview()
  } catch {
    /* stopPreview 抛错不阻断清理 */
  }
  void store.flushPendingSave()
  // 退出编辑器时关闭已打开的剧本——下次从主菜单进入时回到剧本列表
  store.closeScript()
  gameStore.initialized = false
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
/* MenuItem 的 .content 默认只有 width:100%，在 .fill（flex 列）里不会收缩 */
.fill :deep(.content) {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
/* 棋盘底纹：透明图片不至于糊成一片黑 */
.asset-thumb,
.char-thumb {
  background:
    repeating-conic-gradient(rgba(255, 255, 255, 0.08) 0% 25%, transparent 0% 50%) 0 0 / 10px 10px;
}
</style>
