<script setup lang="ts">
import { computed, reactive } from 'vue'
import { Toggle } from '@/components/base'
import { useScriptEditorStore } from '@/stores/modules/script-editor'
import { createScript } from '@/api/services/script-editor'

const store = useScriptEditorStore()

const props = defineProps<{ modal: 'script' | 'chapter' | 'character' | 'importChar' | null }>()
const emit = defineEmits<{ 'update:modal': [value: 'script' | 'chapter' | 'character' | 'importChar' | null] }>()

const close = () => emit('update:modal', null)

const importForm = reactive({ folders: new Set<string>(), withAvatar: false })

const MODAL_TITLES: Record<string, string> = {
  script: '新建剧本',
  chapter: '新建章节',
  character: '新建角色',
  importChar: '从全局角色库导入',
}
const modalTitle = computed(() => MODAL_TITLES[props.modal ?? ''] ?? '')

const scriptForm = reactive({
  folderName: '',
  description: '',
  isAdventure: false,
  boundCharacterFolder: '',
})
const chapterForm = reactive({ id: '', name: '' })
const charForm = reactive({ folder: '', aiName: '', systemPrompt: '' })

const confirmModal = async () => {
  const which = props.modal
  emit('update:modal', null)
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
    if (importForm.folders.size === 0) return
    for (const folder of importForm.folders) {
      await store.importGlobalCharacter(folder, importForm.withAvatar)
    }
    importForm.folders.clear()
  }
}
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
        v-if="modal"
        class="modal-mask fixed inset-0 z-[9999] flex items-center justify-center p-4 backdrop-blur-md bg-black/55"
        @click.self="close"
      >
        <!-- 主弹窗 -->
        <div class="w-[min(440px,92vw)] max-h-[86vh] overflow-y-auto border border-white/12.5 rounded-xl py-4 px-[18px] pb-[18px] bg-[rgba(12,20,30,0.86)] backdrop-blur-lg backdrop-saturate-[1.4] shadow-[0_8px_32px_rgba(0,0,0,0.45),inset_0_1px_1px_rgba(255,255,255,0.06)]">
          <div class="flex items-center gap-2 border-b-2 border-brand pb-2 mb-4">
            <h4 class="font-semibold text-white">{{ modalTitle }}</h4>
            <button
              class="ml-auto text-white/50 transition-all duration-300 hover:text-brand hover:rotate-90"
              @click="close"
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
                importForm.folders.has(g.folder) && !g.alreadyInScript ? '!border-brand bg-brand/20 ring-1 ring-brand/30' : '',
              ]"
              @click="g.alreadyInScript ? null : importForm.folders.has(g.folder) ? importForm.folders.delete(g.folder) : importForm.folders.add(g.folder)"
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
              <span
                v-if="importForm.folders.has(g.folder)"
                class="ml-auto text-xs text-brand"
                >已选 ✓</span
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
              @click="close"
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
</template>
