<script setup lang="ts">
  import { computed, reactive } from "vue";
  import { useI18n } from "vue-i18n";
  import { Toggle } from "@/components/base";
  import { useScriptEditorStore } from "@/stores/modules/script-editor";
  import { createScript } from "@/api/services/script-editor";

  const { t } = useI18n();
  const store = useScriptEditorStore();

  const props = defineProps<{ modal: "script" | "chapter" | "character" | "importChar" | null }>();
  const emit = defineEmits<{
    "update:modal": [value: "script" | "chapter" | "character" | "importChar" | null];
  }>();

  const close = () => emit("update:modal", null);

  const importForm = reactive({ folders: new Set<string>(), withAvatar: false });

  const MODAL_TITLES: Record<string, string> = {
    script: "scriptEditor.editorModals.newScript",
    chapter: "scriptEditor.editorModals.newChapter",
    character: "scriptEditor.editorModals.newCharacter",
    importChar: "scriptEditor.editorModals.importCharacter",
  };
  const modalTitle = computed(() => {
    const key = MODAL_TITLES[props.modal ?? ""];
    return key ? t(key) : "";
  });

  const scriptForm = reactive({
    folderName: "",
    description: "",
    isAdventure: false,
    boundCharacterFolder: "",
  });
  const chapterForm = reactive({ id: "", name: "" });
  const charForm = reactive({ folder: "", aiName: "", systemPrompt: "" });

  const confirmModal = async () => {
    const which = props.modal;
    emit("update:modal", null);
    if (which === "script") {
      try {
        const pkg = await createScript({ ...scriptForm });
        Object.assign(scriptForm, {
          folderName: "",
          description: "",
          isAdventure: false,
          boundCharacterFolder: "",
        });
        await store.refreshScripts();
        await store.openScript(pkg.key);
      } catch (e) {
        store.notifyError(t("scriptEditor.notify.newScriptFailed"), e);
      }
    } else if (which === "chapter") {
      await store.createChapter(chapterForm.id, chapterForm.name);
      chapterForm.id = "";
      chapterForm.name = "";
    } else if (which === "character") {
      await store.createCharacter(charForm.folder, charForm.aiName, charForm.systemPrompt);
      Object.assign(charForm, { folder: "", aiName: "", systemPrompt: "" });
    } else if (which === "importChar") {
      if (importForm.folders.size === 0) return;
      for (const folder of importForm.folders) {
        await store.importGlobalCharacter(folder, importForm.withAvatar);
      }
      importForm.folders.clear();
    }
  };
</script>

<template>
  <!-- Teleport 到 #app 而非 body：body 下浮层脱离 #app 的整体缩放作用域，
       缩放后与编辑器主体比例失调（详见 PreviewStage 注释） -->
  <Teleport to="#app">
    <Transition
      enter-active-class="transition-opacity duration-200 ease"
      leave-active-class="transition-opacity duration-200 ease"
      enter-from-class="opacity-0"
      leave-to-class="opacity-0"
    >
      <div
        v-if="modal"
        class="modal-mask fixed inset-0 z-[9999] flex items-center justify-center bg-black/55 p-4
          backdrop-blur-md"
        @click.self="close"
      >
        <!-- 主弹窗 -->
        <div
          class="max-h-[86dvh] w-[min(440px,92vw)] overflow-y-auto rounded-xl border
            border-white/12.5 bg-[rgba(12,20,30,0.86)] px-[18px] py-4 pb-[18px]
            shadow-[0_8px_32px_rgba(0,0,0,0.45),inset_0_1px_1px_rgba(255,255,255,0.06)]
            backdrop-blur-lg backdrop-saturate-[1.4]"
        >
          <div class="border-brand mb-4 flex items-center gap-2 border-b-2 pb-2">
            <h4 class="font-semibold text-white">{{ modalTitle }}</h4>
            <button
              class="hover:text-brand ml-auto text-white/50 transition-all duration-300
                hover:rotate-90"
              @click="close"
            >
              ✕
            </button>
          </div>

          <template v-if="modal === 'script'">
            <div class="mb-4">
              <label class="text-brand inline-flex items-center text-[0.9rem] font-medium">{{
                t("scriptEditor.editorModals.scriptName")
              }}</label>
              <input
                v-model="scriptForm.folderName"
                class="glass-input"
                :placeholder="t('scriptEditor.editorModals.descriptionPlaceholder')"
              />
              <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40">
                {{ t("scriptEditor.editorModals.scriptNameHint") }}
              </p>
            </div>
            <div class="mb-4">
              <label class="text-brand inline-flex items-center text-[0.9rem] font-medium">{{
                t("scriptEditor.editorModals.description")
              }}</label>
              <textarea v-model="scriptForm.description" class="glass-input min-h-16"></textarea>
            </div>
            <label
              class="inline-flex items-center gap-2 text-[0.8rem] whitespace-nowrap text-white/70"
            >
              <Toggle
                :checked="scriptForm.isAdventure"
                @change="(v: boolean) => (scriptForm.isAdventure = v)"
              />
              {{ t("scriptEditor.editorModals.isAdventure") }}
            </label>
            <div v-if="scriptForm.isAdventure" class="mt-2 mb-4">
              <label class="text-brand inline-flex items-center text-[0.9rem] font-medium">{{
                t("scriptEditor.editorModals.boundCharacter")
              }}</label>
              <!-- 下拉直选全局角色库的人物，不手写目录名；角色多时浏览器自带滚动 -->
              <select v-model="scriptForm.boundCharacterFolder" class="glass-input">
                <option value="" disabled>
                  {{ t("scriptEditor.editorModals.selectCharacter") }}
                </option>
                <option v-for="g in store.globalCharacters" :key="g.folder" :value="g.folder">
                  {{ g.aiName }}（{{ g.folder }}）
                </option>
              </select>
              <p
                v-if="store.globalCharacters.length === 0"
                class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-yellow-200"
              >
                {{ t("scriptEditor.editorModals.emptyCharacters") }}
              </p>
            </div>
          </template>

          <template v-else-if="modal === 'chapter'">
            <div class="mb-4">
              <label class="text-brand inline-flex items-center text-[0.9rem] font-medium">{{
                t("scriptEditor.editorModals.chapterFileName")
              }}</label>
              <input
                v-model="chapterForm.id"
                class="glass-input"
                :placeholder="t('scriptEditor.editorModals.chapterIdHint')"
              />
            </div>
            <div class="mb-4">
              <label class="text-brand inline-flex items-center text-[0.9rem] font-medium">{{
                t("scriptEditor.editorModals.displayName")
              }}</label>
              <input
                v-model="chapterForm.name"
                class="glass-input"
                :placeholder="t('scriptEditor.editorModals.chapterNameExample')"
              />
              <p class="mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40">
                {{ t("scriptEditor.editorModals.chapterEndNote") }}
              </p>
            </div>
          </template>

          <!-- 从全局角色库导入 -->
          <template v-else-if="modal === 'importChar'">
            <p
              v-if="store.globalCharacters.length === 0"
              class="py-8 text-center text-[0.85rem] text-white/45"
            >
              {{ t("scriptEditor.editorModals.emptyCharacters") }}
            </p>
            <div
              v-for="g in store.globalCharacters"
              :key="g.folder"
              :class="[
                `mb-1.5 flex items-baseline gap-2 rounded-lg border bg-white/5 px-[11px] py-[9px]
                transition-all duration-150`,
                g.alreadyInScript
                  ? 'cursor-default border-white/10 opacity-45'
                  : `hover:border-brand cursor-pointer border-white/10
                    hover:bg-[rgba(121,217,255,0.08)]`,
                importForm.folders.has(g.folder) && !g.alreadyInScript
                  ? '!border-brand bg-brand/20 ring-brand/30 ring-1'
                  : '',
              ]"
              @click="
                g.alreadyInScript
                  ? null
                  : importForm.folders.has(g.folder)
                    ? importForm.folders.delete(g.folder)
                    : importForm.folders.add(g.folder)
              "
            >
              <span class="font-semibold text-white">{{ g.aiName }}</span>
              <code class="text-brand font-mono">{{ g.folder }}</code>
              <span v-if="g.alreadyInScript" class="ml-auto text-xs text-white/35">{{
                t("scriptEditor.editorModals.alreadyInScript")
              }}</span>
              <span v-else-if="!g.hasAvatar" class="ml-auto text-xs text-yellow-200">{{
                t("scriptEditor.editorModals.noAvatar")
              }}</span>
              <span v-if="importForm.folders.has(g.folder)" class="text-brand ml-auto text-xs">{{
                t("scriptEditor.editorModals.selected")
              }}</span>
            </div>

            <label
              class="mt-3 inline-flex items-center gap-2 text-[0.8rem] whitespace-nowrap
                text-white/70"
            >
              <Toggle
                :checked="importForm.withAvatar"
                @change="(v: boolean) => (importForm.withAvatar = v)"
              />
              {{ t("scriptEditor.editorModals.copyAvatar") }}
            </label>
            <p
              class="[&_code]:text-brand mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40
                [&_code]:font-mono"
              v-html="t('scriptEditor.editorModals.avatarCopyHint')"
            ></p>
          </template>

          <template v-else>
            <div class="mb-4">
              <label class="text-brand inline-flex items-center text-[0.9rem] font-medium">{{
                t("scriptEditor.editorModals.characterFolder")
              }}</label>
              <input
                v-model="charForm.folder"
                class="glass-input"
                :placeholder="t('scriptEditor.editorModals.characterFolderHint')"
              />
            </div>
            <div class="mb-4">
              <label class="text-brand inline-flex items-center text-[0.9rem] font-medium">{{
                t("scriptEditor.editorModals.displayName")
              }}</label>
              <input v-model="charForm.aiName" class="glass-input" />
            </div>
            <div class="mb-4">
              <label class="text-brand inline-flex items-center text-[0.9rem] font-medium">{{
                t("scriptEditor.editorModals.characterPrompt")
              }}</label>
              <textarea
                v-model="charForm.systemPrompt"
                class="glass-input min-h-24"
                :placeholder="t('scriptEditor.editorModals.characterPromptHint')"
              ></textarea>
            </div>
            <p
              class="[&_code]:text-brand mt-[0.3rem] text-[0.72rem] leading-[1.7] text-white/40
                [&_code]:font-mono"
              v-html="t('scriptEditor.editorModals.avatarPlaceHint')"
            ></p>
          </template>

          <div class="mt-5 flex justify-end gap-2">
            <button
              class="inline-flex items-center gap-1 rounded-lg border border-white/10 bg-white/6
                px-3 py-[0.3rem] text-[0.8rem] whitespace-nowrap text-white/70 transition-all
                duration-200 hover:enabled:bg-white/[0.12] hover:enabled:text-white
                disabled:cursor-not-allowed disabled:opacity-40"
              @click="close"
            >
              {{ t("scriptEditor.imageCrop.cancel") }}
            </button>
            <button
              class="border-brand/45 text-brand bg-brand/14 hover:bg-brand/24 inline-flex
                items-center gap-1 rounded-lg border px-3 py-[0.3rem] text-[0.8rem]
                whitespace-nowrap transition-all duration-200 disabled:cursor-not-allowed
                disabled:opacity-40"
              :disabled="
                modal === 'script' && scriptForm.isAdventure && !scriptForm.boundCharacterFolder
              "
              @click="confirmModal"
            >
              {{ t("scriptEditor.editorModals.confirm") }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
